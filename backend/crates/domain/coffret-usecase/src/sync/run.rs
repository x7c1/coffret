use coffret_model::Redacted;
use tracing::{info, warn};

use crate::commit::catch_up;
use crate::departure::{discard_departed, EmptiedFolders};
use crate::progress::{Phase, Step};
use crate::relocate::{relocate_waiting, LocalDisk, Relocation};
use crate::rename::RenamedPath;
use crate::spooled_container::commit_spooled;
use crate::sync::disposal::Disposal;
use crate::sync::settled::Settled;
use crate::sync::surfaced::Surfaced;
use crate::sync::sync_error::SyncResult;
use crate::sync::sync_outcome::SyncOutcome;
use crate::sync::sync_request::SyncRequest;
use crate::sync::{scan, settle, spool, stale_scratches};
use crate::upload;

/// Carries every changed file under this device's mapped folders into the
/// Library.
///
/// The whole path, in the order it has to happen in: catch the Index up to the
/// Library's head (spec: CK-9), settle the pending rows an interrupted run left
/// (spec: OC-2, OC-3, OC-7), scan the mapped folders against the Index
/// (spec: EP-9, EP-10), encode what is new or changed into Containers of its own
/// (spec: FM-1, FM-2, FM-3, FM-4, FM-5, FM-6, FM-7, FM-8, FM-9, PK-15), spool
/// and upload them (spec: OC-2), and commit the batch (spec: CP-1). What the
/// Library becomes is decided by
/// [`commit_batch`](crate::commit::commit_batch), and everything before it
/// changes nothing about committed Catalog membership: before the Journal
/// record, spools and uploaded objects are accounted for by this device's
/// pending rows. An uncertain create outcome is retained until proven.
///
/// The catch-up comes first and its failure fails the run, for the reason every
/// other use case catches up before it reads the catalog, save an Entry fetch
/// answering from a catalog its process keeps current: what the Index says
/// about an Entry Path is an answer about the Library only where it stands at
/// the Library's head. A scan over a catalog that stands behind it takes files
/// the Library already holds for new ones — every file under the mappings, on a
/// catalog that was discarded and not yet rebuilt — spools and uploads them, and
/// then meets the Entries already current at those paths as EP-6 collisions when
/// the commit catches the Index up. It costs one listing of Storage per run,
/// which is what reading the head takes (spec: FM-12), and it buys back every
/// upload a catalog behind the head would otherwise repeat — after a discard,
/// and equally after another device committed what this one is about to carry.
/// The commit's own catch-up stays where it is, as the guard against a head that
/// moved while this run was walking folders (spec: CP-2).
///
/// Settling comes next, and still before the scan, because the scan reads what
/// the settling decides. A row of this device's own is either a batch that never
/// committed or a commit whose record landed and whose Index refresh did not,
/// and the Library-wide half of that refresh is what the catch-up has just
/// replayed while the device-local half exists only in the row (spec: OC-7). A
/// scan that ran with it open would find a current Entry at a path with no local
/// row behind it, read the path as one this device never materialized, and pass
/// silently over every later change to that file (spec: EP-10).
///
/// Two kinds of file are reported and not acted on: one whose current Entry
/// lives in a Pack, and one this device had and no longer has. Neither is
/// skipped quietly, and neither is an error — so a caller reads
/// [`SyncOutcome::surfaced`]. A run that returns successfully with findings in
/// it has *not* backed up every local file (spec: PK-14).
///
/// A mapping whose local root the device cannot vouch for is reported the same
/// way, in [`SyncOutcome::unavailable`]: for a root that is not there, or one
/// that is empty while standing on a filesystem the mapping does not record,
/// nothing under it is walked and no deletion under it is inferred, because an
/// unplugged disk must never read as the user having emptied the folder
/// (spec: EP-12). The device's other mappings scan normally.
///
/// A file this device placed whose Entry a rename has given another path is
/// relocated to follow it after the settling and before the scan, with one
/// rename inside its mapped root, and its record follows it; one that cannot be
/// relocated — its root unavailable, the file edited, something of the person's at
/// the new path — is left where it stands and reported in
/// [`SyncOutcome::relocations`], and the next run tries again (spec: EP-10).
///
/// A file this device materialized that is gone, paired with the one file that
/// appeared under the same mapped root holding exactly its Entry's content, is
/// a file renamed on disk: the batch renames the Entry to where the file now
/// stands, uploads nothing for it, and reports it in [`SyncOutcome::renamed`]
/// rather than as a deletion and an addition. A pair the Library would refuse
/// to rename is read as those two instead, and the refusal is reported beside
/// them (spec: EP-10, FM-15).
///
/// A file this device materialized whose Entry has since left the Library is
/// not carried back in as new (spec: EP-15). One that still holds what this
/// device last made it match is discarded into the desktop's trash and its row
/// is forgotten; one that changed is kept and reported by every run; a discard
/// that is refused leaves the file and its row where they are, is reported with
/// what refused it, and is tried again by the next run — and fails nothing
/// else.
///
/// A run with nothing to upload or rename commits nothing rather than
/// committing an empty batch: a Journal record is a generation, and creating
/// one for a batch that changes nothing would make every device replay a record
/// that says nothing (spec: CP-1).
///
/// The spools of a committed batch are removed at the end, and a removal that
/// fails there is recorded and not raised. The Library has already changed and
/// the commit's own refresh has already dropped the rows, so failing the run
/// over a file that would not go would report a sync that did not happen; what
/// is left is ciphertext no row names, which is orphan cleanup's to find
/// (spec: OC-1, OC-4). The outcome carries the commit either way — the same
/// posture [`fetch`](crate::fetch) takes over the scratches a failed
/// placement leaves.
pub async fn sync_folders(request: SyncRequest<'_>) -> SyncResult<SyncOutcome> {
    let SyncRequest {
        store,
        index,
        keys,
        spool: local,
        roots,
        trash,
        relocations,
        spool_dir,
        batch,
        now,
        progress,
        policy,
        births,
    } = request;

    let _pending_owner = index.own_pending_rows().await?;

    // Before the settling and the scan alike, because both read the catalog and
    // neither may read one standing behind the Library's head (spec: CK-9).
    //
    // Said before it starts rather than counted through it: what the catch-up
    // has to replay is known only as it is read, and on a device that has just
    // joined that is the whole Journal. It is the first minutes of the run and
    // nothing else would say a word during them.
    progress.step(Step::begun(Phase::CatchingUp));
    let caught = catch_up(store, index, keys.control(), &policy.retry).await?;
    // What the scan reads the catalog at, and so what the batch is prepared
    // against (spec: CP-4, EP-7).
    let base = caught.head_generation();

    // The settle announces its own phase, and only where there are rows to
    // settle. It reads the slots an interrupted batch attempted through the
    // listing the catch-up has just walked (spec: OC-3).
    let settled = settle::settle(
        settle::SettleStorage {
            store,
            keys: keys.control(),
            caught: &caught,
        },
        index,
        local,
        &policy,
        now,
        progress,
    )
    .await?;

    // The scan is what produces the count the packing phase reports, so it has
    // none of its own to give: a folder's files are known once it has walked
    // them.
    progress.step(Step::begun(Phase::Scanning));
    // The files of Entries a rename gave another path follow them first, so
    // the scan finds each where its Entry stands. After the settling, which can
    // record a file waiting to follow its Entry, and never through a root the
    // run cannot vouch for (spec: EP-10, EP-12).
    let relocations = relocate_waiting(
        index,
        LocalDisk {
            roots,
            relocations,
            trash,
        },
    )
    .await?;
    let mut survey = scan::scan(index, roots, births, now).await?;

    // Only now, with the scan over: every departed file stands under a root the
    // walk could read (spec: EP-12), and the discard holds that root against its
    // mapping before it writes (spec: EP-13). Before the encoding, so that what
    // went to the trash is settled whatever becomes of the batch.
    for path in std::mem::take(&mut survey.forgotten) {
        index.forget_local_entry(&path).await?;
    }
    let mut emptied = EmptiedFolders::default();
    for departed in std::mem::take(&mut survey.departed) {
        let root = departed.source.root.clone();
        let expected = departed.expected;
        let relative = departed.source.relative.clone();
        let surfaced = discard_departed(index, trash, departed).await?;
        if matches!(surfaced, Surfaced::Discarded { .. }) {
            emptied.vacated(&root, expected, &relative);
        }
        survey.surfaced.push(surfaced);
    }
    // Once every discard is made, so a folder several departed files stood in
    // is asked about after the last of them left it.
    emptied.remove(trash).await;
    stale_scratches::remove(trash, std::mem::take(&mut survey.stale)).await;
    local.prepare_dir(&spool_dir).await?;

    // The scan has just said how much there is to do, and the encoding is the
    // first thing that takes long enough to be worth saying anything about.
    let to_pack = survey.candidates.len();
    progress.step(Step::new(Phase::Packing, 0, to_pack));
    let mut spooled = Vec::with_capacity(to_pack);
    for (done, candidate) in survey.candidates.iter().enumerate() {
        spooled.push(
            spool::spool(
                index, keys, local, roots, &spool_dir, &batch, now, candidate,
            )
            .await?,
        );
        progress.step(Step::new(Phase::Packing, done + 1, to_pack));
    }
    upload::upload(
        store,
        index,
        local,
        &policy.retry,
        &batch,
        progress,
        &mut spooled,
    )
    .await?;

    // What this device saw of a file it did not have to upload is its own
    // bookkeeping, and belongs to it whether or not this run commits anything
    // (spec: EP-10).
    for observation in survey.refreshed {
        index.mark_present(observation).await?;
    }

    // Nothing to speak for: a sync never reads the committed Keyring, so it
    // carries no finding about it into the commit.
    let renames = std::mem::take(&mut survey.renames);
    let commit = commit_spooled(
        store,
        index,
        keys.control(),
        &policy,
        now,
        base,
        &spooled,
        &[],
        renames.clone(),
        Vec::new(),
        None,
        progress,
    )
    .await?;
    let renamed: Vec<RenamedPath> = renames
        .into_iter()
        .map(|rename| RenamedPath {
            from: rename.from,
            to: rename.path,
        })
        .collect();
    if commit.is_some() && !renamed.is_empty() {
        // The record has given each Entry the path its file already stands at,
        // so the record of the file follows it there and waits for nothing.
        // Only after the commit: before it the Entry stood at the old path. An
        // interruption between the two leaves the file at the new path agreeing
        // with the old record, which the next run's relocation re-keys without
        // renaming anything (spec: EP-10).
        let rekeyed: Vec<_> = renamed
            .iter()
            .map(|one| (one.from.clone(), one.to.clone()))
            .collect();
        index.relocate_local_entries(&rekeyed).await?;
    }
    if commit.is_some() {
        // The commit's refresh has already dropped their pending rows
        // (spec: OC-2), so the ciphertext on this device is the last thing
        // left of the batch.
        for container in &spooled {
            if let Err(error) = local.discard(&container.spool_path).await {
                warn!(
                    container = %container.container_id,
                    reason = %error.redacted(),
                    "a committed Container's spool file could not be removed",
                );
            }
        }
    }

    let outcome = SyncOutcome {
        added: spooled
            .iter()
            .map(|container| container.container_id)
            .collect(),
        replaced: spooled
            .iter()
            .flat_map(|container| container.replaces.iter().copied())
            .collect(),
        renamed,
        unchanged: survey.unchanged,
        // Read here rather than taken from the scan, because it is an answer
        // about the device and not about what this run walked: a device that
        // maps nothing walks nothing whatever is on its disk, and those are
        // two different empty answers (spec: EP-9).
        mappings: index.mappings().await?.len(),
        relocations,
        surfaced: survey.surfaced,
        reserved: survey.reserved,
        unavailable: survey.unavailable,
        settled,
        commit,
    };
    let completed = outcome
        .settled
        .iter()
        .filter(|one| matches!(one, Settled::Completed { .. }))
        .count();
    // Among the disposed, the ones whose object Storage would not trash: the
    // one settle outcome that leaves something behind on Storage.
    let left_in_storage = outcome
        .settled
        .iter()
        .filter(|one| {
            matches!(
                one,
                Settled::Disposed {
                    disposal: Disposal::LeftInStorage { .. },
                    ..
                }
            )
        })
        .count();
    info!(
        added = outcome.added.len(),
        replaced = outcome.replaced.len(),
        renamed = outcome.renamed.len(),
        unchanged = outcome.unchanged,
        // A count and nothing else: a mapping names a prefix and a local root,
        // and neither may reach a diagnostic event.
        mappings = outcome.mappings,
        relocated = outcome
            .relocations
            .iter()
            .filter(|one| matches!(one, Relocation::Relocated { .. }))
            .count(),
        relocations_refused = outcome
            .relocations
            .iter()
            .filter(|one| matches!(one, Relocation::Refused { .. }))
            .count(),
        surfaced = outcome.surfaced.len(),
        reserved = outcome.reserved.len(),
        discarded = outcome
            .surfaced
            .iter()
            .filter(|one| matches!(one, Surfaced::Discarded { .. }))
            .count(),
        // A count and nothing else: the prefix is an Entry Path component and
        // the root is a local path, and neither may reach a diagnostic event.
        unavailable = outcome.unavailable.len(),
        completed,
        disposed = outcome
            .settled
            .iter()
            .filter(|one| matches!(one, Settled::Disposed { .. }))
            .count(),
        retained = outcome
            .settled
            .iter()
            .filter(|one| matches!(one, Settled::Retained { .. }))
            .count(),
        left_in_storage,
        "a sync run finished",
    );
    Ok(outcome)
}
