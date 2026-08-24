use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, EntryMetadata};
use tracing::{debug, info, warn};

use crate::commit::{catch_up, CommitPolicy, ControlKeys};
use crate::device_state::{DeviceTime, LocalObservation, PendingUpload};
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync::reconciled::Reconciled;
use crate::sync::spool;
use crate::sync::sync_error::SyncResult;

/// Settles what an interrupted run left behind, before this one reads a byte of
/// local state.
///
/// # The two things a pending row can turn out to be
///
/// A row names a Container this device encoded, and perhaps uploaded, before any
/// commit (spec: OC-2). What a caught-up Index says about that Container decides
/// which of two opposite things happened, and the answer is never ambiguous
/// because a replayed record is never unlearned (spec: CP-1):
///
/// - **Not current.** The batch never committed. The spool goes, the object goes
///   to the provider's trash where there is one, and the row goes with them
///   (spec: OC-2, OC-3).
/// - **Current.** The batch's record landed and this device's own
///   [`Index::refresh`] is what did not. The Library-wide half of that refresh is
///   the record, which the catch-up has just replayed; the device-local half is
///   *only* here, so it is completed from the row rather than reclaimed — the
///   Entries the Container holds become present (spec: EP-10), and the spool
///   and the row are dropped because the Container they account for is
///   committed (spec: OC-7).
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
/// # Why it runs first, and what the head read costs
///
/// Both verdicts rest on an Index that has read the Library's head, and this run
/// reads it itself rather than waiting for one that commits. Waiting is what
/// produced the failure this exists to prevent: a run that scanned with a row of
/// its own unsettled would find no current Entry at a path this device has
/// already committed, spool and upload the file a second time, and meet its own
/// Entry as an EP-6 collision when the commit caught the Index up.
///
/// The head read is paid only where it decides something. A run with no pending
/// rows — every run, in the ordinary case — asks the Index one question and stops
/// (spec: OC-6). A row that names no object needs no head either: nothing that
/// was never uploaded can be current, so its spool and its row go whatever the
/// Library holds. What is left is a row with an object behind it, and there the
/// catch-up failing fails the run: carrying on would mean scanning with the
/// question still open. Such a failure reaches the caller as
/// [`SyncError::Commit`](crate::sync::SyncError::Commit), batchless run and all.
pub(super) async fn reconcile(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    now: DeviceTime,
) -> SyncResult<Vec<Reconciled>> {
    // What this run is about to commit is not among these: a row is written
    // while spooling and dropped by the commit's own refresh (spec: OC-2), so
    // what is here belongs to a run that ended before that.
    let pending = index.pending_uploads().await?;
    if pending.is_empty() {
        return Ok(Vec::new());
    }

    if pending.iter().any(|row| row.object_ref.is_some()) {
        catch_up(store, index, keys, &policy.retry).await?;
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
        reconciled.push(if current.contains(&row.container_id) {
            let entries = landed.remove(&row.container_id);
            complete(index, now, row, entries).await?
        } else {
            dispose(store, index, policy, row).await?
        });
    }
    Ok(reconciled)
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
/// of where these rows come from and nowhere else: a pending row is written by
/// the spool step alone, for a one-file Container this flow's own scan drew from
/// a local file (spec: PK-15), so the one Entry it holds is a file this device
/// put on disk. What a commit adds is otherwise no evidence of that — a repack
/// commits Containers whose Entries the device may never have held, which is why
/// [`CommittedBatch`](crate::CommittedBatch) names the materialized files rather
/// than leaving them to be read off the additions.
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
        .map(|row| row.container_id)
        .filter(|container_id| current.contains(container_id))
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

    spool::discard(&row.spool_path).await?;
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
/// Container is absent from the current set, read off an Index the caller caught
/// up to the head wherever a row named an object at all (spec: OC-3). So a row
/// that names one has its object trashed, and a current Container never reaches
/// this — its bookkeeping is completed instead, and trashing it here would take
/// an object the Library holds out of Storage.
async fn dispose(
    store: &dyn ObjectStore,
    index: &dyn Index,
    policy: &CommitPolicy,
    row: PendingUpload,
) -> SyncResult<Reconciled> {
    spool::discard(&row.spool_path).await?;

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
                    reason = %error,
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
