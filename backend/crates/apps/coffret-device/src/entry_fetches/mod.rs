//! One fetch per Entry Path at a time, for a process that serves more than one
//! reader.
//!
//! A command line asks for one Entry and waits for it, so nothing there can ask
//! twice at once. A server can: a reader that opens a page and prefetches the
//! next one, two tabs on one folder, a browser retrying a request it thinks
//! stalled — all of them arrive as two requests for one Entry Path, overlapping.
//!
//! Both would run the whole flow. Both would catch the catalog up, read the
//! committed Keyring, range-read the same extent of the same Container, write a
//! temporary file, and rename onto one path — the second one over a file the
//! first had already placed and marked present. Nothing is corrupted by that,
//! because a rename is atomic and each file is fully verified before it happens
//! (spec: EP-11); what is spent is the Container read twice, and what is
//! confused is the device's own bookkeeping, which briefly says a file was
//! materialized twice at one moment.
//!
//! So the second caller waits, and then asks the cheapest question there is:
//! whether the file is there now. Where the first caller placed it, the second
//! answers [`AlreadyPresent`](EntryFetch::AlreadyPresent) out of the catalog
//! alone, having read nothing from Storage. Where the first caller declined the
//! path, the second runs the flow and arrives at the same finding by itself —
//! the wasted read being the price of not caching a verdict about a folder that
//! anything on this device may have changed in the meantime.
//!
//! It is a property of the process rather than of the Library, which is why it
//! is a value a process holds and not a field of
//! [`OpenLibrary`](crate::OpenLibrary): two Libraries open in one process have
//! nothing to coordinate, and one Library open in two processes cannot be
//! coordinated from here anyway.

use coffret_model::EntryPath;
use coffret_usecase::fetch::EntryFetch;
use tracing::debug;

mod gates;
use gates::Gates;

use crate::browse::EntryState;
use crate::error::Result;
use crate::open_library::OpenLibrary;

/// The Entry Paths this process is fetching right now.
#[derive(Debug, Default)]
pub struct EntryFetches {
    in_flight: Gates<EntryPath>,
}

impl EntryFetches {
    /// Nothing in flight.
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes the Entry at `path` available, waiting for whoever is already
    /// fetching it.
    ///
    /// The same three answers [`OpenLibrary::fetch_entry`] gives, and the same
    /// meaning for each: this call adds who goes first and nothing else
    /// (spec: EP-11).
    pub async fn fetch(&self, library: &OpenLibrary, path: EntryPath) -> Result<EntryFetch> {
        let _turn = self.in_flight.take(path.clone()).await;

        // Asked after the wait rather than before it, which is what makes it
        // worth asking: whoever went first may have placed the file, and the
        // catalog is where that shows (spec: EP-10).
        if library.state_of(&path).await? == EntryState::Present {
            debug!(
                operation = "fetch_entry",
                verdict = "already present",
                "another caller had fetched this Entry",
            );
            return Ok(EntryFetch::AlreadyPresent);
        }
        library.fetch_entry(path).await
    }
}
