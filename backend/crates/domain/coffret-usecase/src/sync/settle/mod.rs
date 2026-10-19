use std::collections::BTreeSet;

use coffret_model::ContainerId;
use tracing::{info, warn};

use crate::commit::{CaughtUp, CommitPolicy, ControlKeys};
use crate::device_state::DeviceTime;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::progress::{Phase, Progress, Step};
use crate::spool::Spool;
use crate::sync::settled::Settled;
use crate::sync::sync_error::SyncResult;

// What the commit slots a batch recorded say about one of its Containers
// (spec: OC-3).
mod attempt;
use attempt::{attempts, judge, Attempt};

// A Container whose commit landed and whose refresh did not (spec: OC-7).
mod complete;
use complete::{complete, completes, materialized};

mod dispose;
use dispose::dispose;

/// What settlement reads Storage with: the run's own catch-up and the keys
/// that authenticate a control object.
pub(super) struct SettleStorage<'a> {
    pub(super) store: &'a dyn ObjectStore,
    pub(super) keys: &'a ControlKeys,
    pub(super) caught: &'a CaughtUp,
}

/// Settles interrupted work while the caller exclusively owns pending rows.
///
/// A current Container completes its refresh (spec: OC-7). A row whose commit
/// was never attempted may be disposed of. A row whose commit was attempted is
/// judged by the commit slots its batch recorded (spec: OC-2, OC-3): a record
/// at one of them naming the Container is its batch's commit, and completes it;
/// an authenticated successor at every one of them, none of them naming it, is
/// proof the batch did not commit, and the row is disposed of like one never
/// attempted. Anything less retains the ciphertext and its provenance, even
/// when catch-up found no record (spec: OC-1, OC-3).
pub(super) async fn settle(
    storage: SettleStorage<'_>,
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
    let attempts = attempts(&storage, index, policy, &pending, &current).await?;

    let mut settled = Vec::with_capacity(pending.len());
    for row in pending {
        settled.push(if completes(&row, &current) {
            let entries = landed.remove(&row.container_id);
            complete(index, spool, now, row, entries).await?
        } else if row.commit_attempted {
            let occupants = attempts
                .get(&row.batch)
                .map(Vec::as_slice)
                .unwrap_or_default();
            match judge(row.container_id, occupants) {
                Attempt::Committed => complete(index, spool, now, row, None).await?,
                Attempt::Lost => {
                    info!(
                        container = %row.container_id,
                        batch = %row.batch,
                        slots = occupants.len(),
                        "every commit slot an interrupted batch attempted holds another record",
                    );
                    dispose(&storage, index, spool, policy, row).await?
                }
                Attempt::Unknown => {
                    warn!(container = %row.container_id, batch = %row.batch,
                        slots = occupants.len(),
                        "retained a Container whose commit outcome is unknown");
                    Settled::Retained {
                        container_id: row.container_id,
                    }
                }
            }
        } else {
            dispose(&storage, index, spool, policy, row).await?
        });
    }
    Ok(settled)
}
