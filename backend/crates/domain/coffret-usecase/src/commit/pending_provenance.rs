use super::{CommitError, PreparedBatch};
use crate::Index;

/// Persist uncertainty before a Journal create can reach Storage (spec: OC-3).
/// A failure here sends no create. The producer holds the pending-row guard.
///
/// The batch's rows are marked in one write, every one of them or none (see
/// [`Index::mark_commit_attempted`]): a batch left half-marked by an
/// interruption here would keep the marked half's spool files and Storage
/// objects forever, although no create was ever sent for them.
pub(super) async fn mark_attempt(
    index: &dyn Index,
    batch: &PreparedBatch,
) -> Result<(), CommitError> {
    let additions: Vec<_> = batch
        .additions
        .iter()
        .map(|addition| addition.addition.container().id)
        .collect();
    index.mark_commit_attempted(&additions).await?;

    Ok(())
}
