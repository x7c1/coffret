//! What `prune` answers with.

use serde::Serialize;

use coffret_device::PruneOutcome;

/// What `prune` did.
///
/// Generations rather than object names for records: the names follow from
/// them (`head-<n>`, `idx-<n>`), and a script comparing two runs compares
/// numbers. A Keyring replica left in place is named in full, since its
/// generation alone does not say which of a set's replicas it is.
#[derive(Serialize)]
pub struct Pruned {
    head: u64,
    snapshot: String,
    wrote_snapshot: bool,
    removed: usize,
    oldest_removed: Option<u64>,
    newest_removed: Option<u64>,
    unremoved: Vec<u64>,
    removed_replicas: usize,
    unremoved_replicas: Vec<String>,
}

impl Pruned {
    /// The answer of a prune that rested on the Snapshot of `head`, removed
    /// the records from `oldest` to `newest` and trashed `removed_replicas`
    /// Keyring replicas, for the golden cases, which cannot assemble a whole
    /// outcome without a Library.
    #[cfg(test)]
    pub(super) fn assembled(
        head: u64,
        wrote_snapshot: bool,
        removed: std::ops::RangeInclusive<u64>,
        unremoved: Vec<u64>,
        removed_replicas: usize,
        unremoved_replicas: Vec<String>,
    ) -> Self {
        Self {
            head,
            snapshot: format!("idx-{head}.cfrt"),
            wrote_snapshot,
            removed: removed.clone().count(),
            oldest_removed: Some(*removed.start()),
            newest_removed: Some(*removed.end()),
            unremoved,
            removed_replicas,
            unremoved_replicas,
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
            removed_replicas: outcome.removed_replicas.len(),
            unremoved_replicas: outcome
                .unremoved_replicas
                .iter()
                .map(|unremoved| unremoved.replica.to_string())
                .collect(),
        }
    }
}
