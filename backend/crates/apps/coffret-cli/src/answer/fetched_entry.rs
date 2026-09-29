//! What `fetch --entry` answers with.

use serde::Serialize;

use coffret_device::EntryFetch;

/// What `fetch --entry` did, which has three answers and the two counts the
/// text form prints for them.
#[derive(Serialize)]
pub struct FetchedEntry {
    entry: &'static str,
    fetched: usize,
    skipped: usize,
}

impl From<&EntryFetch> for FetchedEntry {
    fn from(fetched: &EntryFetch) -> Self {
        let (entry, fetched, skipped) = match fetched {
            EntryFetch::Placed => ("placed", 1, 0),
            EntryFetch::AlreadyPresent => ("already_present", 0, 1),
            EntryFetch::Surfaced(_) => ("surfaced", 0, 0),
        };
        Self {
            entry,
            fetched,
            skipped,
        }
    }
}
