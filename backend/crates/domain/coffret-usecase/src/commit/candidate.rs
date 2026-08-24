use std::collections::BTreeSet;

use coffret_model::{ContainerId, EntryPath};

use crate::commit::commit_error::{CommitError, CommitResult};
use crate::commit::prepared_batch::PreparedBatch;
use crate::index::Index;

/// Refuses a batch whose commit would put two current Entries at one Entry Path
/// (spec: EP-5, EP-6).
///
/// The order is the one EP-6 fixes and it is not incidental: every Entry owned
/// by the batch's removals leaves the current path map first, and the additions
/// enter after. That is what lets a path move from a replaced Container to its
/// replacement inside one batch, and it is why a collision with a Container the
/// batch is removing is no collision at all.
///
/// This runs before anything is written, because a batch that cannot commit
/// should not have left a Keyring candidate behind for orphan cleanup to reason
/// about. A writer that later loses the commit race runs it again against the
/// new head, which is how two concurrent writes to one path become an explicit
/// conflict instead of last-write-wins (spec: CP-7, EP-7).
pub(super) async fn check(index: &dyn Index, batch: &PreparedBatch) -> CommitResult<()> {
    let removed: BTreeSet<ContainerId> = batch.removals.iter().copied().collect();
    let mut paths: BTreeSet<EntryPath> = index
        .entries_under(None)
        .await?
        .into_iter()
        .filter(|location| !removed.contains(&location.container_id))
        .map(|location| location.entry.path)
        .collect();

    for prepared in &batch.additions {
        for entry in &prepared.addition.entries {
            if !paths.insert(entry.path.clone()) {
                return Err(CommitError::EntryPathCollision {
                    path: entry.path.clone(),
                });
            }
        }
    }
    Ok(())
}
