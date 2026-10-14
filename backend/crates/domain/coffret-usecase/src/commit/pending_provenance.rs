use super::{CommitError, PreparedBatch};
use crate::device_state::AttemptedSlot;
use crate::Index;

/// Persist uncertainty before a Journal create can reach Storage (spec: OC-2,
/// OC-3). A failure here sends no create. The producer holds the pending-row
/// guard.
///
/// The batch's rows are marked, and the slot the create is about to consume is
/// recorded for the batch, in one write, all of it or none (see
/// [`Index::mark_commit_attempted`]): a batch left half-marked by an
/// interruption here would keep the marked half's spool files and Storage
/// objects forever, although no create was ever sent for them, and a marked row
/// without its slot could never be proven not to have committed (spec: OC-3).
pub(super) async fn mark_attempt(
    index: &dyn Index,
    batch: &PreparedBatch,
    slot: &AttemptedSlot,
) -> Result<(), CommitError> {
    let additions: Vec<_> = batch
        .additions
        .iter()
        .map(|addition| addition.addition.container().id)
        .collect();
    index.mark_commit_attempted(&additions, slot).await?;

    Ok(())
}
