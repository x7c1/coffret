//! What `fetch --entry` answers with.

use serde::Serialize;

use coffret_device::{EntryFetch, EntryFetchOutcome};

/// What `fetch --entry` did, which has four answers, the two counts the text
/// form prints for them, and how many other files came with the Entry.
///
/// `alongside` is the text form's "placed alongside N": a parcel is read whole,
/// so the other files lying wholly inside the parcels read are placed too
/// (spec: PK-16), and a caller counting what is on disk counts them.
#[derive(Serialize)]
pub struct FetchedEntry {
    entry: &'static str,
    fetched: usize,
    skipped: usize,
    alongside: usize,
}

impl From<&EntryFetchOutcome> for FetchedEntry {
    fn from(outcome: &EntryFetchOutcome) -> Self {
        let (entry, fetched, skipped) = match outcome.fetch {
            EntryFetch::Placed => ("placed", 1, 0),
            EntryFetch::AlreadyPresent => ("already_present", 0, 1),
            EntryFetch::Surfaced(_) => ("surfaced", 0, 0),
            // A command line waits for its Entry and cancels nothing, so this is
            // never what it hears; named for what it would mean all the same
            // (spec: PK-21).
            EntryFetch::Cancelled => ("cancelled", 0, 0),
        };
        Self {
            entry,
            fetched,
            skipped,
            alongside: outcome.alongside.len(),
        }
    }
}
