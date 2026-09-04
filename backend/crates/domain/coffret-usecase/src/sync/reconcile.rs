use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, EntryMetadata, Redacted};
use tracing::{debug, info, warn};

use crate::commit::CommitPolicy;
use crate::device_state::{DeviceTime, LocalObservation, PendingUpload, SpoolState};
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::spool_file;
use crate::sync::reconciled::Reconciled;
use crate::sync::sync_error::SyncResult;

/// Settles what an interrupted run left behind, before this one reads a byte of
/// local state.
///
/// The name predates the split of the two acts "reconcile" once covered: this is
/// the *settle* act (spec: OC-7), not the *rebase* of a losing writer's batch
/// onto the new head (spec: CP-4).
///
/// # The three things a pending row can turn out to be
///
/// A row names a Container this device was about to write, wrote, or uploaded
/// before any commit (spec: OC-2). Its own state and what a caught-up Index says
/// about that Container together decide which of three things happened, and the
/// answer is never ambiguous because a replayed record is never unlearned
/// (spec: CP-1):
///
/// - **A spool that was never finished.** The row says
///   [`Spooling`](crate::device_state::SpoolState::Spooling), so the run
///   died between announcing the file and marking it `Spooled`. What is at the
///   path may be nothing at all, part of a Container, or a whole one the run
///   never got to mark — but it was certainly never uploaded, since only a spool
///   whose row calls it `Spooled` is ever uploaded, so no current set can name it
///   and it is disposed of whatever the Library holds.
/// - **A finished spool whose batch was abandoned.** The Container is not
///   current, so the batch never committed. The spool goes, the object goes to
///   the provider's trash where there is one, and the row goes with them
///   (spec: OC-2, OC-3).
/// - **A batch that committed while its refresh did not.** The Container is
///   current: the batch's record landed and this device's own
///   [`Index::refresh`] is what did not. The Library-wide half of that refresh is
///   the record, which the catch-up has just replayed; the device-local half is
///   *only* here, so it is completed from the row rather than reclaimed — the
///   Entries the Container holds become present (spec: EP-10), and the spool
///   and the row are dropped because the Container they account for is
///   committed (spec: OC-7).
///
/// The first two are disposed of identically. Only the third is completed.
///
/// # Why a spool is never resumed into a batch
///
/// A Container is opened by a Container Key drawn for it alone (spec: KD-2), and
/// the one place that key is ever written down is the Key Envelope the commit
/// puts in the Keyring (spec: FM-14, KL-7) — which is exactly the step an
/// interrupted run did not reach. So a spool an earlier run left behind is
/// ciphertext with no key anywhere on this device or on Storage: resuming it into
/// a new batch would commit a Container nothing can ever open, and a second place
/// to keep key material is not a trade worth making to avoid re-encrypting a
/// file.
///
/// What a run does instead is dispose of it and let its own scan spool the source
/// file again. That converges for the reason it needs to: the Entry ends up
/// committed exactly once, under a Container the committed Keyring maps, and
/// neither the spool file nor the row survives to be found a third time.
///
/// # Where the head it reads comes from
///
/// The verdict that can go either way rests on an Index that has read the
/// Library's head, and this reads none itself:
/// [`sync_folders`](crate::sync::sync_folders) catches the catalog up before
/// anything reads it (spec: CK-9), so a row's Container is measured here against
/// the Library as it stands. Nothing is asked of Storage at all except the
/// trashing of an object no record names, which is the one thing a settle
/// changes outside this device (spec: OC-3).
///
/// # Why it runs before the scan
///
/// The device-local half of an interrupted refresh exists in the pending row
/// alone: the record the catch-up replayed makes the Container current again,
/// but nothing in it says this device materialized those Entries. A scan that ran
/// with the row still open would find a current Entry at a path with no local
/// row behind it, read the path as one this device never materialized, and pass
/// silently over every later modification and deletion of that file
/// (spec: EP-10).
pub(super) async fn reconcile(
    store: &dyn ObjectStore,
    index: &dyn Index,
    policy: &CommitPolicy,
    now: DeviceTime,
) -> SyncResult<Vec<Reconciled>> {
    // What this run is about to commit is not among these: a row is written just
    // before the spool file it names and dropped by the commit's own refresh
    // (spec: OC-2), so what is here belongs to a run that ended before that.
    let pending = index.pending_uploads().await?;
    if pending.is_empty() {
        return Ok(Vec::new());
    }

    let current: BTreeSet<ContainerId> = index
        .containers_under(None)
        .await?
        .into_iter()
        .map(|container| container.id)
        .collect();
    let mut landed = materialized(index, &pending, &current).await?;

    let mut reconciled = Vec::with_capacity(pending.len());
    for row in pending {
        reconciled.push(if completes(&row, &current) {
            let entries = landed.remove(&row.container_id);
            complete(index, now, row, entries).await?
        } else {
            dispose(store, index, policy, row).await?
        });
    }
    Ok(reconciled)
}

/// Whether one row is the commit-landed-refresh-did-not case rather than
/// something to reclaim.
///
/// Both halves of the test have to hold, and the state half is not implied by
/// the membership half. A row still
/// [`Spooling`](crate::device_state::SpoolState::Spooling) is a spool this
/// device announced and never finished, so nothing ever uploaded it and no record
/// can name it: a current Container of that ID would be some other run's, and
/// completing this row against it would mark Entries present that this device
/// never put on disk (spec: EP-10). Disposing of it instead is OC-2's posture
/// over this device's own provenance.
///
/// One function rather than a test written twice, because
/// [`materialized`] has to pick out exactly the rows the loop will complete —
/// two spellings of that could drift into a walk that gathers Entries nothing
/// consumes, or a completion with no Entries to record.
fn completes(row: &PendingUpload, current: &BTreeSet<ContainerId>) -> bool {
    row.state == SpoolState::Spooled && current.contains(&row.container_id)
}

/// The current Entries of every pending Container that turned out to be current.
///
/// These are the files the interrupted run put on disk while producing the
/// batch: the record carries each new Container's entry table (spec: CP-11), and
/// its `path`, `size`, and `mtime` are exactly what that run handed the refresh
/// it never completed. Reading them out of the Index rather than off Storage is
/// what keeps this from opening a Container.
///
/// Taking the Container's Entries *as* the materialized files is sound because
/// of where these rows come from and nowhere else: a pending row is written by a
/// spool step alone, for a Container this device built out of local files it
/// holds — a one-file Container a sync drew from one of them, or a Pack a freeze
/// drew from several (spec: PK-7, PK-15) — so every Entry it holds is a file
/// this device put on disk. What a commit adds is otherwise no evidence of that
/// — a repack commits Containers whose Entries the device may never have held,
/// which is why [`CommittedBatch`](crate::CommittedBatch) names the materialized
/// files rather than leaving them to be read off the additions.
///
/// One walk of the current Entries answers every completion, and it is walked at
/// all only where there is one to answer: the whole listing is not a hot path
/// when the ordinary run leaves this function unreached.
async fn materialized(
    index: &dyn Index,
    pending: &[PendingUpload],
    current: &BTreeSet<ContainerId>,
) -> SyncResult<BTreeMap<ContainerId, Vec<EntryMetadata>>> {
    let completing: BTreeSet<ContainerId> = pending
        .iter()
        .filter(|row| completes(row, current))
        .map(|row| row.container_id)
        .collect();
    let mut entries: BTreeMap<ContainerId, Vec<EntryMetadata>> = BTreeMap::new();
    if completing.is_empty() {
        return Ok(entries);
    }
    for location in index.entries_under(None).await? {
        if completing.contains(&location.container_id) {
            entries
                .entry(location.container_id)
                .or_default()
                .push(location.entry);
        }
    }
    Ok(entries)
}

/// Completes the bookkeeping of a commit that landed and whose refresh did not
/// (spec: OC-7).
///
/// The object is the Library's and is left exactly where it is. Everything else
/// the interrupted refresh would have done is done here: the Entries the
/// Container holds are marked present, stamped with this run's clock because the
/// moment the earlier run looked is not recorded anywhere (spec: EP-10), and the
/// spool and the row go because a committed Container is no longer a candidate
/// for cleanup (spec: OC-2).
async fn complete(
    index: &dyn Index,
    now: DeviceTime,
    row: PendingUpload,
    entries: Option<Vec<EntryMetadata>>,
) -> SyncResult<Reconciled> {
    let entries = entries.unwrap_or_default();
    for entry in &entries {
        index
            .mark_present(LocalObservation {
                path: entry.path.clone(),
                size: entry.size,
                mtime: entry.mtime,
                at: now,
            })
            .await?;
    }

    spool_file::discard(&row.spool_path).await?;
    index.clear_pending_upload(row.container_id).await?;
    info!(
        container = %row.container_id,
        batch = %row.batch,
        entries = entries.len(),
        "completed the bookkeeping of a Container whose commit landed and whose refresh did not",
    );
    Ok(Reconciled::Completed {
        container_id: row.container_id,
        entries: entries.len(),
    })
}

/// Deletes one abandoned spool, and its object where nothing names it.
///
/// That nothing names it is settled before the call and not re-asked here: the
/// Container is absent from the current set, read off an Index the run caught up
/// to the Library's head before any of this (spec: OC-3). So a row that names one
/// has its object trashed, and a current Container whose spool was finished never
/// reaches this — its bookkeeping is completed instead, and trashing it here
/// would take an object the Library holds out of Storage.
///
/// A row whose spool was never finished lands here too, and needs no special
/// case. It carries no object, so nothing is trashed; and the file it names may
/// be whole, half-written, or absent, all three of which
/// [`spool_file::discard`] treats alike.
async fn dispose(
    store: &dyn ObjectStore,
    index: &dyn Index,
    policy: &CommitPolicy,
    row: PendingUpload,
) -> SyncResult<Reconciled> {
    spool_file::discard(&row.spool_path).await?;

    let trashed = match &row.object_ref {
        Some(object) => match policy.retry.run("trash", || store.trash(object)).await {
            Ok(()) => {
                info!(
                    container = %row.container_id,
                    batch = %row.batch,
                    "trashed a Container an interrupted run uploaded and never committed",
                );
                true
            }
            // The row is about to go, so the provenance this rested on goes
            // with it; what is left is an object no current state names, which
            // is orphan cleanup's to find and a person's to decide on
            // (spec: OC-1, OC-4).
            Err(error) => {
                warn!(
                    container = %row.container_id,
                    reason = %error.redacted(),
                    "an abandoned Container is still in Storage",
                );
                false
            }
        },
        None => {
            debug!(
                container = %row.container_id,
                batch = %row.batch,
                "an interrupted run's spool never left the device",
            );
            false
        }
    };

    index.clear_pending_upload(row.container_id).await?;
    Ok(Reconciled::Disposed {
        container_id: row.container_id,
        trashed,
    })
}
