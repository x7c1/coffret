use coffret_model::{ControlObjectName, Generation};

use crate::prune::unremoved_record::UnremovedRecord;

/// What a prune did.
///
/// What it rested on, and what it removed. A run that removed nothing — the
/// Library's Journal holds only its head, or a prune a moment earlier took
/// the rest — is an ordinary success, and [`removed`](Self::removed) is empty.
#[derive(Debug)]
pub struct PruneOutcome {
    /// The head the run caught up to, which it kept (spec: CK-4).
    pub head: Generation,
    /// The ordinary Index Snapshot of that head the run rested on: `idx-<head>`
    /// (spec: CK-10).
    pub snapshot: ControlObjectName,
    /// Whether this run wrote that Snapshot, rather than finding the one
    /// another run had written (spec: CK-8, CK-11).
    pub wrote_snapshot: bool,
    /// The last Journal generation the Snapshot applies, at or before which
    /// records were eligible (spec: CK-1, CK-4).
    pub covered_through: Generation,
    /// The generations of the records this run moved to the trash, oldest
    /// first (spec: CK-6).
    pub removed: Vec<Generation>,
    /// The eligible generations whose records were already gone when the run
    /// came to them: another device pruned them first.
    pub already_gone: Vec<Generation>,
    /// The eligible head positions holding something other than a Journal
    /// record — an activation Snapshot — which the run kept (spec: CK-6).
    pub kept: Vec<Generation>,
    /// The records it could not remove, each with its reason.
    ///
    /// Those it did remove stay removed, and the next prune takes these.
    pub unremoved: Vec<UnremovedRecord>,
}

impl PruneOutcome {
    /// The oldest and the newest generation the run removed, or `None` where
    /// it removed nothing.
    ///
    /// A range in the sense a person reads it, not a promise that every
    /// generation between the two was removed by this run: one another device
    /// took first, or one Storage would not trash, can stand inside it.
    pub fn removed_range(&self) -> Option<(Generation, Generation)> {
        Some((*self.removed.first()?, *self.removed.last()?))
    }
}
