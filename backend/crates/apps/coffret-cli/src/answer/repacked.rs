//! What `repack` answers with.

use serde::Serialize;

use coffret_device::RepackOutcome;

use super::committed_head;

/// What `repack` did.
///
/// Counts of Packs and the bytes sent: a Container ID names nothing a person
/// or a script acts on here, and the folder is the one it typed.
#[derive(Serialize)]
pub struct Repacked {
    selected: usize,
    kept: usize,
    removed: usize,
    added: usize,
    uploaded: u64,
    committed_head: Option<u64>,
}

impl Repacked {
    /// The answer of a repack with these counts, for the golden cases, which
    /// cannot assemble a whole outcome without a Library.
    #[cfg(test)]
    pub(super) fn assembled(
        selected: usize,
        kept: usize,
        removed: usize,
        added: usize,
        uploaded: u64,
        committed_head: Option<u64>,
    ) -> Self {
        Self {
            selected,
            kept,
            removed,
            added,
            uploaded,
            committed_head,
        }
    }
}

impl From<&RepackOutcome> for Repacked {
    fn from(outcome: &RepackOutcome) -> Self {
        Self {
            selected: outcome.selected.len(),
            kept: outcome.kept.len(),
            removed: outcome.removed.len(),
            added: outcome.added.len(),
            uploaded: outcome.uploaded(),
            committed_head: committed_head(outcome.commit.as_ref()),
        }
    }
}
