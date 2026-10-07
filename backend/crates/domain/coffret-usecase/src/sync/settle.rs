use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, EntryMetadata, Redacted};
use tracing::{debug, info, warn};

use crate::commit::CommitPolicy;
use crate::device_state::{DeviceTime, LocalObservation, PendingRow};
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::progress::{Phase, Progress, Step};
use crate::spool::Spool;
use crate::sync::disposal::Disposal;
use crate::sync::settled::Settled;
use crate::sync::sync_error::SyncResult;

/// Settles interrupted work while the caller exclusively owns pending rows.
/// A current Container completes its refresh. A row whose commit was never
/// attempted may be disposed of; uncertain attempts keep both the ciphertext
/// and its provenance, even when catch-up found no record (spec: OC-1, OC-3).
pub(super) async fn settle(
    store: &dyn ObjectStore,
    index: &dyn Index,
    spool: &dyn Spool,
    policy: &CommitPolicy,
    now: DeviceTime,
    progress: &dyn Progress,
) -> SyncResult<Vec<Settled>> {
    // What this run is about to commit is not among these: a row is written just
    // before the spool file it names and dropped by the commit's own refresh
    // (spec: OC-2), so what is here belongs to a run that ended before that.
    let pending = index.pending_rows().await?;
    if pending.is_empty() {
        return Ok(Vec::new());
    }
    progress.step(Step::begun(Phase::Settling));

    let current: BTreeSet<ContainerId> = index
        .containers_under(None)
        .await?
        .into_iter()
        .map(|container| container.id)
        .collect();
    let mut landed = materialized(index, &pending, &current).await?;

    let mut settled = Vec::with_capacity(pending.len());
    for row in pending {
        settled.push(if completes(&row, &current) {
            let entries = landed.remove(&row.container_id);
            complete(index, spool, now, row, entries).await?
        } else if row.commit_attempted {
            warn!(container = %row.container_id, batch = %row.batch,
                "retained a Container whose commit outcome is unknown");
            Settled::Retained {
                container_id: row.container_id,
            }
        } else {
            dispose(store, index, spool, policy, row).await?
        });
    }
    Ok(settled)
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
fn completes(row: &PendingRow, current: &BTreeSet<ContainerId>) -> bool {
    row.state.is_spooled() && current.contains(&row.container_id)
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
    pending: &[PendingRow],
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
    spool: &dyn Spool,
    now: DeviceTime,
    row: PendingRow,
    entries: Option<Vec<EntryMetadata>>,
) -> SyncResult<Settled> {
    let entries = entries.unwrap_or_default();
    for entry in &entries {
        index
            .mark_present(LocalObservation {
                path: entry.path.clone(),
                size: entry.extent.size(),
                mtime: entry.mtime,
                at: now,
            })
            .await?;
    }

    spool.discard(&row.spool_path).await?;
    index.clear_pending_row(row.container_id).await?;
    info!(
        container = %row.container_id,
        batch = %row.batch,
        entries = entries.len(),
        "completed the bookkeeping of a Container whose commit landed and whose refresh did not",
    );
    Ok(Settled::Completed {
        container_id: row.container_id,
        entries: entries.len(),
    })
}

/// Disposes only of provenance proven abandoned before any commit attempt.
/// The caller holds exclusive ownership, so no live producer can resume it.
async fn dispose(
    store: &dyn ObjectStore,
    index: &dyn Index,
    spool: &dyn Spool,
    policy: &CommitPolicy,
    row: PendingRow,
) -> SyncResult<Settled> {
    spool.discard(&row.spool_path).await?;

    let disposal = match row.state.object_ref() {
        Some(object) => match policy.retry.run("trash", || store.trash(object)).await {
            Ok(()) => {
                info!(
                    container = %row.container_id,
                    batch = %row.batch,
                    "trashed a Container an interrupted run uploaded and never committed",
                );
                Disposal::Trashed
            }
            // Retain the row on refusal: it remains the proof that makes
            // retrying this cleanup safe (spec: OC-2, OC-3).
            Err(error) => {
                warn!(
                    container = %row.container_id,
                    batch = %row.batch,
                    reason = %error.redacted(),
                    "an abandoned Container is still in Storage",
                );
                Disposal::LeftInStorage { cause: error }
            }
        },
        None => {
            debug!(
                container = %row.container_id,
                batch = %row.batch,
                "an interrupted run's spool never left the device",
            );
            Disposal::NeverUploaded
        }
    };

    if !matches!(disposal, Disposal::LeftInStorage { .. }) {
        index.clear_pending_row(row.container_id).await?;
    }
    Ok(Settled::Disposed {
        container_id: row.container_id,
        disposal,
    })
}
