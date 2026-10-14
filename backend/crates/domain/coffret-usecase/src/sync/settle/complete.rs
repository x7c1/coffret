use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, EntryMetadata};
use tracing::info;

use crate::device_state::{DeviceTime, LocalObservation, PendingRow};
use crate::index::Index;
use crate::spool::Spool;
use crate::sync::settled::Settled;
use crate::sync::sync_error::SyncResult;

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
pub(super) fn completes(row: &PendingRow, current: &BTreeSet<ContainerId>) -> bool {
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
/// spool step alone, and one that says it
/// [`materializes`](PendingRow::materializes) is for a Container this device
/// built out of local files it holds — a one-file Container a sync drew from
/// one of them, or a Pack a freeze drew from several (spec: PK-7, PK-15) — so
/// every Entry it holds is a file this device put on disk. A Pack a deletion
/// rebuilt by read-modify-replace carries Entries forward off Storage instead
/// (spec: PK-10), and its row says so, so it contributes none.
///
/// What a commit adds is otherwise no evidence of that — a repack commits
/// Containers whose Entries the device may never have held, which is why
/// [`CommittedBatch`](crate::CommittedBatch) names the materialized files
/// rather than leaving them to be read off the additions.
///
/// One walk of the current Entries answers every completion, and it is walked at
/// all only where there is one to answer: the whole listing is not a hot path
/// when the ordinary run leaves this function unreached.
pub(super) async fn materialized(
    index: &dyn Index,
    pending: &[PendingRow],
    current: &BTreeSet<ContainerId>,
) -> SyncResult<BTreeMap<ContainerId, Vec<EntryMetadata>>> {
    let completing: BTreeSet<ContainerId> = pending
        .iter()
        .filter(|row| completes(row, current) && row.materializes)
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
/// the interrupted refresh would have done is done here: the `entries` it is
/// handed, if any, are marked present, stamped with this run's clock because the
/// moment the earlier run looked is not recorded anywhere (spec: EP-10), and the
/// spool and the row go because a committed Container is no longer a candidate
/// for cleanup (spec: OC-2).
pub(super) async fn complete(
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
                hash: Some(entry.hash),
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
