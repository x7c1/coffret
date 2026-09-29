//! What `freeze` answers with.

use serde::Serialize;

use coffret_device::FreezeOutcome;

use super::committed_head;

/// What `freeze` did.
#[derive(Serialize)]
pub struct Frozen {
    packs: usize,
    entries: usize,
    absorbed: usize,
    packed_already: usize,
    committed_head: Option<u64>,
    mappings: usize,
}

impl From<&FreezeOutcome> for Frozen {
    fn from(outcome: &FreezeOutcome) -> Self {
        Self {
            packs: outcome.packs.len(),
            entries: outcome.frozen_entries(),
            absorbed: outcome.absorbed.len(),
            packed_already: outcome.packed_already,
            committed_head: committed_head(outcome.commit.as_ref()),
            mappings: outcome.mappings,
        }
    }
}
