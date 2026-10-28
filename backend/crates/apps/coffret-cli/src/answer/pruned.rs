//! What `prune` answers with.

use serde::Serialize;

use coffret_device::PruneOutcome;

/// What `prune` did.
///
/// Generations rather than object names: the names follow from them
/// (`head-<n>`, `idx-<n>`), and a script comparing two runs compares numbers.
#[derive(Serialize)]
pub struct Pruned {
    head: u64,
    snapshot: String,
    wrote_snapshot: bool,
    removed: usize,
    oldest_removed: Option<u64>,
    newest_removed: Option<u64>,
    unremoved: Vec<u64>,
}

impl Pruned {
    /// The answer of a prune that rested on the Snapshot of `head` and removed
    /// the records from `oldest` to `newest`, for the golden cases, which
    /// cannot assemble a whole outcome without a Library.
    #[cfg(test)]
    pub(super) fn assembled(
        head: u64,
        wrote_snapshot: bool,
        removed: std::ops::RangeInclusive<u64>,
        unremoved: Vec<u64>,
    ) -> Self {
        Self {
            head,
            snapshot: format!("idx-{head}.cfrt"),
            wrote_snapshot,
            removed: removed.clone().count(),
            oldest_removed: Some(*removed.start()),
            newest_removed: Some(*removed.end()),
            unremoved,
        }
    }
}

impl From<&PruneOutcome> for Pruned {
    fn from(outcome: &PruneOutcome) -> Self {
        let range = outcome.removed_range();
        Self {
            head: outcome.head.get(),
            snapshot: outcome.snapshot.to_string(),
            wrote_snapshot: outcome.wrote_snapshot,
            removed: outcome.removed.len(),
            oldest_removed: range.map(|(oldest, _)| oldest.get()),
            newest_removed: range.map(|(_, newest)| newest.get()),
            unremoved: outcome
                .unremoved
                .iter()
                .map(|unremoved| unremoved.generation.get())
                .collect(),
        }
    }
}
