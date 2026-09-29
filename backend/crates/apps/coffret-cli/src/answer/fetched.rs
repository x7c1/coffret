//! What `fetch` answers with.

use serde::Serialize;

use coffret_device::FetchOutcome;

/// What `fetch` did.
#[derive(Serialize)]
pub struct Fetched {
    fetched: usize,
    containers: usize,
    skipped: usize,
    mappings: usize,
}

impl From<&FetchOutcome> for Fetched {
    fn from(outcome: &FetchOutcome) -> Self {
        Self {
            fetched: outcome.fetched.len(),
            containers: outcome.containers.len(),
            skipped: outcome.skipped,
            mappings: outcome.mappings,
        }
    }
}
