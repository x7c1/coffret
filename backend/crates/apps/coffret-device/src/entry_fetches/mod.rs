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
//! scratch, and rename onto one path — the second one over a file the
//! first had already placed and marked present. Nothing is corrupted by that,
//! because a rename is atomic and each file is fully verified before it happens
//! (spec: EP-11); what is spent is the Container read twice, and what is
//! confused is the device's own bookkeeping, which briefly says a file was
//! materialized twice at one moment.
//!
//! So the second caller waits, and then asks the cheapest question there is:
//! whether the file is there now. Where the first caller placed it, the second
//! answers [`AlreadyPresent`](EntryFetch::AlreadyPresent) out of the catalog and
//! the file it names, having read nothing from Storage. Where the first caller
//! declined the path, the second runs the flow and arrives at the same finding
//! by itself — the wasted read being the price of not caching a verdict about a
//! folder that anything on this device may have changed in the meantime.
//!
//! It is a property of the process rather than of the Library, which is why it
//! is a value a process holds and not a field of
//! [`OpenLibrary`](crate::OpenLibrary): two Libraries open in one process have
//! nothing to coordinate, and one Library open in two processes cannot be
//! coordinated from here anyway.

use coffret_model::{EntryPath, Redacted};
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
        //
        // The row is not the whole of the question, because a row outlives the
        // file somebody deleted out of the mapped folder. So the shortcut asks
        // for the row *and* a file standing at the path it names. That is not
        // EP-11's *agreement*, which holds the record's length and modification
        // time against the disk's and is the selection's question rather than
        // this one; what is settled here is only whether there is anything to
        // serve. Where there is not, the flow runs and states the finding,
        // rather than this answering "already present" about a file that is not
        // there and leaving the caller with nothing to say.
        if library.state_of(&path).await? == EntryState::Present
            && placed_file_stands(library, &path).await
        {
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

/// Whether the file the row names can be opened right now.
///
/// The second half of the shortcut's question, and the one answer it is allowed
/// to settle is `true`. Every way of not being able to say so is `false` here
/// rather than a failure of the call: the file gone is the case the question is
/// asked for, and a link on the way down to it, a path no mapping reaches any
/// more, or a row the Library holds no current Entry at are each a verdict the
/// flow below owns and states in its own vocabulary. This is a shortcut past
/// that flow and not a second place to reach one of its verdicts, so it declines
/// to take itself and lets the flow answer.
///
/// Raising here instead would cost the caller that sentence twice over: it
/// would reach a reader as "the server could not answer" about a path the flow
/// would have named a reason for, and it would reach a caller filling a folder
/// as a failure about this device rather than about one Entry — the reading
/// that stops such a run on every file it had left. What being wrong costs is
/// one run of the flow instead, which is the price this module already puts on
/// not caching a verdict about a folder anything on this device may have
/// changed.
async fn placed_file_stands(library: &OpenLibrary, path: &EntryPath) -> bool {
    match library.open_local_file(path).await {
        Ok(standing) => standing.is_some(),
        // Not swallowed: the flow is about to ask the same question of the same
        // path and say what is wrong with it, and this is the one line saying
        // the shortcut met it first. The Entry Path stays out of the event as it
        // does everywhere else (spec: EL-1), and the cause goes in redacted.
        Err(cause) => {
            debug!(
                operation = "fetch_entry",
                verdict = "no shortcut",
                error = %cause.redacted(),
                "the placed file would not open, so the flow is what states why",
            );
            false
        }
    }
}
