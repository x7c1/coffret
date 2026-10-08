use std::collections::BTreeSet;

use coffret_model::{ContainerId, EntryPath};

use crate::commit::commit_error::{CommitError, CommitResult};
use crate::commit::prepared_batch::PreparedBatch;
use crate::index::Index;

/// Refuses a batch that cannot commit onto the head the Index stands at: one
/// that removes a Container that is no longer current (spec: CP-18), or one
/// whose commit would put two current Entries at one Entry Path (spec: EP-5,
/// EP-6).
///
/// The removals are checked first, because the path check takes them as given:
/// it lets an Entry leave with a Container the batch removes, and a Container
/// another writer already removed has no Entries left to leave. A batch
/// prepared against a head where that Container was current would otherwise
/// pass, and its commit would silently undo the other writer's removal — a
/// replacement landing for a file somebody deleted, a rebuilt Pack bringing
/// back Entries somebody deleted with it (spec: CP-7).
///
/// The path order is the one EP-6 fixes and it is not incidental: every Entry
/// owned by the batch's removals leaves the current path map first, and the
/// additions enter after. That is what lets a path move from a replaced
/// Container to its replacement inside one batch, and it is why a collision
/// with a Container the batch is removing is no collision at all.
///
/// This runs before anything is written, because a batch that cannot commit
/// should not have left a Keyring candidate behind for orphan cleanup to reason
/// about. A writer that later loses the commit race runs it again against the
/// new head, which is how two concurrent writes become an explicit conflict
/// instead of last-write-wins (spec: CP-7, CP-18, EP-7).
pub(super) async fn check(index: &dyn Index, batch: &PreparedBatch) -> CommitResult<()> {
    let removed: BTreeSet<ContainerId> = batch.removals.iter().copied().collect();
    check_removals(index, &removed).await?;

    let mut paths: BTreeSet<EntryPath> = index
        .entries_under(None)
        .await?
        .into_iter()
        .filter(|location| !removed.contains(&location.container_id))
        .map(|location| location.entry.path)
        .collect();

    for prepared in &batch.additions {
        for entry in prepared.addition.entries() {
            if !paths.insert(entry.path.clone()) {
                return Err(CommitError::EntryPathCollision {
                    path: entry.path.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Refuses removals that name a Container the current set no longer holds,
/// naming every one of them (spec: CP-18).
///
/// A batch that removes nothing costs no walk of the Index.
async fn check_removals(index: &dyn Index, removed: &BTreeSet<ContainerId>) -> CommitResult<()> {
    if removed.is_empty() {
        return Ok(());
    }
    let current: BTreeSet<ContainerId> = index
        .containers_under(None)
        .await?
        .into_iter()
        .map(|container| container.id)
        .collect();
    let gone: Vec<ContainerId> = removed.difference(&current).copied().collect();
    if gone.is_empty() {
        Ok(())
    } else {
        Err(CommitError::RemovalNotCurrent {
            container_ids: gone,
        })
    }
}
