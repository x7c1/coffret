use super::{CommitError, PreparedBatch};
use crate::Index;
use std::collections::BTreeSet;

/// Persist uncertainty before a Journal create can reach Storage (spec: OC-3).
/// A failure here sends no create. Even a partial marker update therefore errs
/// on the side of retaining data. The producer holds the pending-row guard.
pub(super) async fn mark_attempt(
    index: &dyn Index,
    batch: &PreparedBatch,
) -> Result<(), CommitError> {
    let additions: BTreeSet<_> = batch
        .additions
        .iter()
        .map(|addition| addition.addition.container().id)
        .collect();
    for mut row in index.pending_rows().await? {
        if additions.contains(&row.container_id) {
            row.commit_attempted = true;
            index.record_pending_row(row).await?;
        }
    }

    Ok(())
}
