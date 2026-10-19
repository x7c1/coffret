use std::collections::BTreeSet;

use coffret_model::{ContainerId, ContainerKeyStatus, Generation, KeyTable, Redacted};
use tracing::{info, warn};

use crate::commit::{catch_up, read_committed, CaughtUp, ControlKeys, ShortKeyring, ShortReport};
use crate::freeze::freeze_error::FreezeResult;
use crate::freeze::freeze_outcome::FreezeOutcome;
use crate::freeze::freeze_request::FreezeRequest;
use crate::freeze::frozen_pack::FrozenPack;
use crate::freeze::scan::{Reserved, Scope};
use crate::freeze::segment::Segment;
use crate::freeze::survey::Closing;
use crate::freeze::{scan, segment, spool};
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::progress::{Phase, Step};
use crate::retry::RetryPolicy;
use crate::spooled_container::{commit_spooled, SpooledContainer};
use crate::sync::{settle, SettleStorage};
use crate::upload;

/// Packs the eligible local files under one folder into Packs (spec: PK-17).
///
/// The whole path, in the order it has to happen in: catch the Index up to the
/// Library's head (spec: CK-9), read the committed Keyring so the run knows
/// which Containers cannot be opened at all (spec: KL-1, KL-7), scan the folder
/// for what this invocation may absorb (spec: PK-1, PK-13, PK-14), sort the
/// selection by Entry Path and cut it into Packs around the target (spec: PK-3,
/// PK-4, PK-6), spool each Pack through the streaming encoder, upload them, and
/// commit one Journal batch (spec: PK-7, CP-1).
///
/// The catch-up comes first because eligibility is a question about the current
/// Library: which Container holds each Entry, and of what kind. A run that
/// answered it from a stale catalog could absorb a one-file Container another
/// device has already replaced, or pass over a file whose Entry is no longer in
/// a Pack.
///
/// What a run may absorb is exactly PK-1's two shapes, and the shape is decided
/// by the Container kind and never by the Entry count (spec: PK-15). A file not
/// yet in the Library becomes a member of a Pack directly — a freeze does not
/// upload a one-file Container first and absorb it afterwards (spec: PK-7). A
/// file whose current Entry is held by a one-file Container is absorbed, and the
/// Pack carries the bytes on disk now, whether the file has been modified or the
/// Container's key has been lost (spec: PK-13). Anything an existing Pack holds
/// is left byte-for-byte as it is (spec: PK-2).
///
/// Two kinds of file are reported and not acted on, and neither is an error: one
/// whose Pack-held Entry the local file no longer matches, and one whose Pack
/// the Library records no key for. Both need `update`'s read-modify-replace over
/// a Pack, which this flow does not do (spec: PK-10, PK-11). Reporting them is
/// not optional — a caller reads [`FreezeOutcome::surfaced`], because a run that
/// returns successfully with findings in it has *not* packed every local file
/// current (spec: PK-14).
///
/// A mapping whose local root the device cannot vouch for is a third such
/// finding, and it is about a mapping rather than a file:
/// [`FreezeOutcome::unavailable`] carries it (spec: EP-12). Nothing under such a
/// root is walked, so it contributes no candidate and can neither absorb nor
/// remove anything; a freeze infers no deletion, so the only harm it could do is
/// silence, which is why the mapping is reported rather than the run refused.
///
/// A run with nothing to pack commits nothing rather than committing an empty
/// batch: a Journal record is a generation, and creating one for a batch that
/// changes no Container would make every device replay a record that says
/// nothing (spec: CP-1). That is also the ordinary second run over a folder —
/// `freeze` persists no folder state, so it simply finds every file already
/// packed (spec: PK-2) — which is exactly why an unavailable root has to be
/// reported: the two produce the same empty answer, and only
/// [`FreezeOutcome::unavailable`] says which one happened.
///
/// The spools of a committed batch are removed at the end, and a removal that
/// fails there is recorded and not raised. The Library has already changed and
/// the commit's own refresh has already dropped the rows, so failing the run
/// over a file that would not go would report a freeze that did not happen; what
/// is left is ciphertext no row names, which is orphan cleanup's to find
/// (spec: OC-1, OC-4). The outcome carries the commit either way.
///
/// What an interrupted run left — this flow's or any other's — is settled
/// before anything is scanned, the way a sync settles it and by the same
/// settlement (spec: OC-2, OC-3, OC-7, OC-9): a pending row naming a Container
/// this device was writing, wrote, or uploaded is disposed of, with its spool
/// and its object, or completed where its batch committed. So a freeze taking
/// up a book a dead process was packing finds no spool of that book's on the
/// disk and no row of it in the catalog by the time it builds its own Pack.
/// There is always a row, whatever stopped the earlier run and wherever it
/// stopped, because the row is written before the spool file it names. A spool
/// is never resumed, because the Container Key that opens it lived only in the
/// run that drew it (spec: KD-2, FM-14) — so the files a dead run packed are
/// simply eligible again.
///
/// The reservations this device holds are read against the scan
/// (spec: PK-17). One every path of which the run considered is decided by it:
/// the commit's refresh releases it where the run packs any of its paths, and
/// it is released once the run has finished, with a [`Lapsed`] per path, where
/// none of them was eligible any more. A run that stops before then leaves
/// every reservation where it was, for the next run to decide. And a run
/// decides no reservation a drop in this process is still writing into, nor one
/// that was live at any moment since the run read them: what it read of such
/// a reservation may be missing pages that landed after, and the drop's own
/// run, armed once it stops receiving, is the one that decides it.
///
/// [`Lapsed`]: crate::freeze::Lapsed
pub async fn freeze_folder(request: FreezeRequest<'_>) -> FreezeResult<FreezeOutcome> {
    let FreezeRequest {
        store,
        index,
        keys,
        spool: local,
        roots,
        spool_dir,
        prefix,
        only,
        target,
        batch,
        now,
        progress,
        policy,
        births,
        live,
    } = request;

    let _pending_owner = index.own_pending_rows().await?;

    // Said before it starts rather than counted through it: what the catch-up
    // has to replay is known only as it is read, and on a device that has just
    // joined that is the whole Journal. It is the first minutes of the run and
    // nothing else would say a word during them.
    progress.step(Step::begun(Phase::CatchingUp));
    let caught = catch_up(store, index, keys.control(), &policy.retry).await?;
    let KeyringFindings {
        base,
        key_lost,
        short,
    } = read_keyring(store, index, keys.control(), &policy.retry, &caught).await?;
    // What an interrupted run left is settled before the scan reads which files
    // are this device's, under the ownership this run already holds and before
    // it has a row of its own: a batch the catch-up found committed is
    // completed, and one that never committed is disposed of with its spool and
    // its object, so the Pack this run builds is the only one of its book
    // anywhere (spec: OC-2, OC-3, OC-7, OC-9). It announces its own phase, and
    // only where there are rows to settle.
    let settled = settle(
        SettleStorage {
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
    // Watched from before the read, so that a drop that starts, lands pages
    // and stops while this run scans is one it spares rather than one it
    // decides from a reading that missed its later pages (spec: PK-17).
    let live = live.since_now();
    let reservations = index.reservations().await?;
    // The read's finding, from here to wherever this run ends. Said as the
    // guard goes out of scope — by the last line of the run or by any `?`
    // before it — unless the commit's examination of the same generation has
    // spoken for it (spec: KL-11, KL-15). One word about a set found short, on
    // every path the run can take.
    let short = ShortReport::armed(short);
    // The scan is what produces the count the packing phase reports, so it has
    // none of its own to give: a folder's files are known once it has walked
    // them.
    progress.step(Step::begun(Phase::Scanning));
    let mut survey = scan::scan(
        index,
        roots,
        births,
        Scope {
            prefix: prefix.as_ref(),
            only: only.as_ref(),
        },
        &key_lost,
        Reserved {
            all: &reservations,
            live: &live,
        },
        now,
    )
    .await?;
    // The reservations the commit packs go with its refresh; the ones none of
    // whose paths was eligible go once the run has finished (spec: PK-17).
    let mut packing = Vec::new();
    let mut lapsing = Vec::new();
    let mut lapsed = Vec::new();
    for closing in std::mem::take(&mut survey.closing) {
        match closing {
            Closing::Packs(id) => packing.push(id),
            Closing::Lapses(id, paths) => {
                lapsing.push(id);
                lapsed.extend(paths);
            }
        }
    }
    let segments = segment::segment(survey.selected, target)?;

    // The cut has just said how many Packs there are, and encoding one is the
    // first thing here that takes long enough to be worth saying anything
    // about.
    let to_pack = segments.len();
    progress.step(Step::new(Phase::Packing, 0, to_pack));
    let mut spooled = Vec::with_capacity(to_pack);
    if !segments.is_empty() {
        local.prepare_dir(&spool_dir).await?;
        for (done, segment) in segments.iter().enumerate() {
            spooled.push(
                spool::spool(index, keys, local, roots, &spool_dir, &batch, now, segment).await?,
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
    }

    // What this device saw of a file it did not have to pack is its own
    // bookkeeping, and belongs to it whether or not this run commits anything
    // (spec: EP-10).
    for observation in survey.refreshed {
        index.mark_present(observation).await?;
    }

    let commit = commit_spooled(
        store,
        index,
        keys.control(),
        &policy,
        now,
        base,
        &spooled,
        &[],
        Vec::new(),
        packing.clone(),
        Some(&short),
        progress,
    )
    .await?;
    // Only once the run has got this far: a reservation released here is one
    // whose files this run looked at and found nothing to pack among, and the
    // findings saying so go out with this outcome (spec: PK-17).
    for id in &lapsing {
        index.release_reservation(*id).await?;
    }
    if commit.is_some() {
        // The commit's refresh has already dropped their pending rows
        // (spec: OC-2), so the ciphertext on this device is the last thing left
        // of the batch.
        for container in &spooled {
            if let Err(error) = local.discard(&container.spool_path).await {
                warn!(
                    container = %container.container_id,
                    reason = %error.redacted(),
                    "a committed Pack's spool file could not be removed",
                );
            }
        }
    }

    let outcome = FreezeOutcome {
        packs: segments
            .iter()
            .zip(&spooled)
            .map(|(segment, container)| built(segment, container, target))
            .collect(),
        absorbed: spooled
            .iter()
            .flat_map(|container| container.replaces.iter().copied())
            .collect(),
        packed_already: survey.packed_already,
        // Read here rather than taken from the scan, because it is an answer
        // about the device and not about what this run walked: a device that
        // maps nothing walks nothing whatever is on its disk, and those are
        // two different empty answers (spec: EP-9).
        mappings: index.mappings().await?.len(),
        surfaced: survey.surfaced,
        unavailable: survey.unavailable,
        settled,
        released: if commit.is_some() {
            packing.len() + lapsing.len()
        } else {
            lapsing.len()
        },
        lapsed,
        commit,
        // Whatever the commit's examination did not speak for, which is all of
        // it where the run had nothing to commit (spec: KL-15).
        short: short.unspoken(),
    };
    info!(
        packs = outcome.packs.len(),
        entries = outcome.frozen_entries(),
        oversized = outcome.packs.iter().filter(|pack| pack.oversized).count(),
        absorbed = outcome.absorbed.len(),
        packed_already = outcome.packed_already,
        // A count and nothing else: a mapping names a prefix and a local root,
        // and neither may reach a diagnostic event.
        mappings = outcome.mappings,
        surfaced = outcome.surfaced.len(),
        // A count and nothing else: the prefix is an Entry Path component and
        // the root is a local path, and neither may reach a diagnostic event.
        unavailable = outcome.unavailable.len(),
        keyring_short = outcome.short.is_some(),
        settled = outcome.settled.len(),
        released = outcome.released,
        lapsed = outcome.lapsed.len(),
        "a freeze run finished",
    );
    Ok(outcome)
}

/// Reads the committed Keyring: which Containers it records no key for
/// (spec: KL-7), and whether the set it was read from is short.
///
/// A freeze has to know which keys are lost, for two opposite reasons. A
/// one-file Container whose key is lost is eligible whatever its content
/// compares to — the stored ciphertext is unreadable, so re-encrypting the
/// local plaintext is the only content-recovery path there is
/// (spec: PK-11, PK-13). A Pack whose key is lost is the same loss and not this
/// flow's to repair, so it is surfaced rather than passed over (spec: PK-14).
///
/// The catch-up before it is what the whole run's eligibility rests on, and
/// reading the Keyring needs the walk of Storage it leaves behind (spec: CK-9,
/// FM-12). A Library that has committed nothing has no Keyring and no current
/// Entry, so there is nothing to be lost and every local file is simply new —
/// and no set to find short either.
///
/// The other half is the degradation, which the run holds in a
/// [`ShortReport`] and says on its way out — from whichever line it leaves
/// by — unless the commit's examination of the same generation has spoken for
/// it: see [`KeyringFindings`] and [`ShortKeyring`].
async fn read_keyring(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    retry: &RetryPolicy,
    caught: &CaughtUp,
) -> FreezeResult<KeyringFindings> {
    let base = caught.head_generation();
    let Some(checkpoint) = index.checkpoint().await? else {
        return Ok(KeyringFindings {
            base,
            ..KeyringFindings::default()
        });
    };
    let read = read_committed(store, keys, retry, &caught.listing, checkpoint.keyring()).await?;
    Ok(KeyringFindings {
        base,
        key_lost: lost(&read.key_table),
        short: read.short,
    })
}

/// What the catch-up and the read of the committed Keyring leave the run with.
///
/// The Keyring's two halves come from the one walk: the Containers the scan
/// has to treat as unreadable, and — where the walk had to step over a position
/// of the set to reach the replica that answered — the finding the run reports
/// if nothing else in it examines that set.
#[derive(Default)]
struct KeyringFindings {
    /// The head the catch-up before the read reached: what the run reads the
    /// catalog at, and so what its batch is prepared against (spec: CP-4,
    /// EP-7).
    base: Option<Generation>,
    /// The Containers the committed Keyring records no key for (spec: KL-7).
    key_lost: BTreeSet<ContainerId>,
    /// The degradation the read found, where it found one.
    short: Option<ShortKeyring>,
}

/// Which Containers of a key table carry a key-lost marker (spec: KL-7).
fn lost(key_table: &KeyTable) -> BTreeSet<ContainerId> {
    key_table
        .elements()
        .iter()
        .filter(|element| element.key == ContainerKeyStatus::KeyLost)
        .map(|element| element.container_id)
        .collect()
}

/// What one Pack came out as.
fn built(segment: &Segment, container: &SpooledContainer, target: u64) -> FrozenPack {
    FrozenPack {
        container_id: container.container_id,
        entries: container.entries.len(),
        footprint: segment.footprint.bytes(),
        oversized: segment.oversized(target),
    }
}
