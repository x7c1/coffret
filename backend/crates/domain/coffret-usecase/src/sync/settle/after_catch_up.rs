use coffret_model::Redacted;
use tracing::{debug, info, warn};

use super::complete::{complete, completes, materialized};
use super::current;
use crate::device_state::DeviceTime;
use crate::index::Index;
use crate::index_error::IndexError;
use crate::spool::Spool;
use crate::sync::sync_error::SyncResult;

/// Who owns this device's pending work while a catch-up's completion runs
/// (spec: OC-2).
pub(crate) enum PendingWork {
    /// The caller owns it for its whole run — a freeze or a deletion holding
    /// the guard [`own_pending_rows`](Index::own_pending_rows) answered with —
    /// and has no row of its own yet.
    Held,
    /// Nobody in this call owns it, so the completion takes ownership for
    /// itself, and leaves the rows alone where a producer or a settlement
    /// holds them.
    Unowned,
}

/// Completes every pending row whose Container a catch-up has just shown
/// current, and nothing else (spec: OC-7).
///
/// A batch whose Journal record landed and whose Index refresh did not is
/// committed whatever this device's catalog said at the time, and once a
/// catch-up has replayed that record the Library-wide half of the refresh is
/// done. The device-local half — the files the batch carried in recorded as
/// present, its spools discarded, its rows removed — needs the rows and the
/// current set and nothing a sync has that a catch-up lacks. Left for the next
/// sync, it would leave those files reading as ones this device never put
/// there: `remote` in a listing, and declined by a fill as another's file
/// standing where the Entry belongs (spec: EP-10). So every catch-up a flow or
/// a process makes on its own is followed by this.
///
/// Only completion runs here. A row whose Container is not current is left
/// exactly as it is for a sync's settlement, because reclaiming it needs the
/// commit slots its batch recorded read back and judged (spec: OC-3), and a row
/// whose spool was never finished is never completed (spec: OC-7).
///
/// The rows are another run's while a producer or a settlement holds them
/// (spec: OC-2): an [`Unowned`](PendingWork::Unowned) call that finds them held
/// does nothing, and the holder's own refresh or settlement finishes them.
///
/// What it answers is how many rows it completed, and it fails nothing. The
/// catch-up has already done what its caller asked for, and every step here is
/// idempotent and leaves the row until its spool is gone (spec: OC-8), so a
/// failure part way leaves a row the next catch-up or sync completes again. It
/// is recorded and the caller goes on with the catalog as it stands, which is
/// where it stood before the completion was tried.
pub(crate) async fn complete_after_catch_up(
    index: &dyn Index,
    spool: &dyn Spool,
    now: DeviceTime,
    work: PendingWork,
) -> usize {
    // Taken before the rows are read, because a competing run may not even read
    // them while a producer owns them (spec: OC-2). Held to the end of the
    // completion and released with it.
    let _taken = match work {
        PendingWork::Held => None,
        PendingWork::Unowned => match index.own_pending_rows().await {
            Ok(guard) => Some(guard),
            Err(IndexError::PendingRowsBusy { .. }) => {
                debug!("another run owns this device's pending work; its rows are left to it");
                return 0;
            }
            Err(error) => {
                warn!(
                    reason = %error.redacted(),
                    "could not take this device's pending work to complete it after a catch-up",
                );
                return 0;
            }
        },
    };
    match complete_current(index, spool, now).await {
        Ok(completed) => completed,
        Err(error) => {
            warn!(
                reason = %error.redacted(),
                "a committed batch's bookkeeping could not be completed after a catch-up; \
                 its rows are left for the next one",
            );
            0
        }
    }
}

/// The completion itself, under ownership the caller has made sure of.
async fn complete_current(
    index: &dyn Index,
    spool: &dyn Spool,
    now: DeviceTime,
) -> SyncResult<usize> {
    let pending = index.pending_rows().await?;
    // The ordinary case, and the one that must cost nothing beyond this read:
    // a run that ended cleanly leaves no row behind (spec: OC-2).
    if pending.is_empty() {
        return Ok(0);
    }
    let current = current(index).await?;
    let mut landed = materialized(index, &pending, &current).await?;
    let mut completed = 0;
    for row in pending {
        if completes(&row, &current) {
            let entries = landed.remove(&row.container_id);
            complete(index, spool, now, row, entries).await?;
            completed += 1;
        }
    }
    if completed > 0 {
        info!(
            completed,
            "completed the bookkeeping of a committed batch after a catch-up",
        );
    }
    Ok(completed)
}
