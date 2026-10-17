//! What `sync` answers with.

use serde::Serialize;

use coffret_device::SyncOutcome;

use super::committed_head;

/// What `sync` did.
#[derive(Serialize)]
pub struct Synced {
    added: usize,
    replaced: usize,
    /// The files renamed on disk that the run carried in as renames of their
    /// Entries, each said once more as a finding of its own.
    renamed: usize,
    unchanged: usize,
    committed_head: Option<u64>,
    mappings: usize,
}

impl From<&SyncOutcome> for Synced {
    fn from(outcome: &SyncOutcome) -> Self {
        Self {
            added: outcome.added.len(),
            replaced: outcome.replaced.len(),
            renamed: outcome.renamed.len(),
            unchanged: outcome.unchanged,
            committed_head: committed_head(outcome.commit.as_ref()),
            mappings: outcome.mappings,
        }
    }
}
