//! What `sync` answers with.

use serde::Serialize;

use coffret_device::SyncOutcome;

use super::committed_head;

/// What `sync` did.
#[derive(Serialize)]
pub struct Synced {
    added: usize,
    replaced: usize,
    unchanged: usize,
    committed_head: Option<u64>,
    mappings: usize,
}

impl From<&SyncOutcome> for Synced {
    fn from(outcome: &SyncOutcome) -> Self {
        Self {
            added: outcome.added.len(),
            replaced: outcome.replaced.len(),
            unchanged: outcome.unchanged,
            committed_head: committed_head(outcome.commit.as_ref()),
            mappings: outcome.mappings,
        }
    }
}
