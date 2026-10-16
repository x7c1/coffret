//! Reading the fronts of a listed folder's Containers before a page of it is
//! opened.
//!
//! The first page of a book not yet read costs three round trips: the
//! Container's header, its meta section, and the parcel the page lies in. The
//! first two are the same whichever page is wanted and name none of them
//! (spec: FM-2, PK-16, PK-20), and a listing is the moment somebody shows they
//! are about to open one. So a listing of a mapped folder reads them, in the
//! background, through the same Storage and into the same memory the Entry
//! fetch uses, and the page opened after costs its parcel alone.
//!
//! The listing never waits for it. It is answered out of the catalog, and what
//! is read here is read after the answer has been handed over, on a task of its
//! own; a read that fails is said and forgotten, because the fetch that wants
//! the front is who answers for it. A page opened while these reads are under
//! way waits for them rather than reading the same objects a second time.

use std::sync::Arc;

use coffret_device::{EntryPath, Redacted};
use tokio::sync::watch;
use tracing::warn;

use crate::state::ServerState;

/// How many reads of fronts are under way, so a case can wait for them rather
/// than sleep on them.
///
/// Nothing about the Library and nothing a browser is told: these reads finish
/// on their own, and whoever opens a page before they have waits for the one
/// read of the listing, key table or front under way rather than making it
/// again.
pub struct FrontsAhead {
    in_flight: watch::Sender<usize>,
}

impl FrontsAhead {
    /// None under way.
    pub fn new() -> Self {
        Self {
            in_flight: watch::Sender::new(0),
        }
    }

    /// Waits until no read of fronts is under way.
    ///
    /// Counted as the read is set going, before its task is spawned, so a case
    /// that has had its listing back waits here for that listing's reads.
    pub async fn until_idle(&self) {
        let mut watched = self.in_flight.subscribe();
        // The sender is a field of the state this was reached through, so it
        // outlives the wait.
        let _ = watched.wait_for(|count| *count == 0).await;
    }
}

impl Default for FrontsAhead {
    fn default() -> Self {
        Self::new()
    }
}

/// One read of fronts under way, ended however its task ends — a panic
/// included, so a case waiting on [`FrontsAhead::until_idle`] is never left
/// waiting on a task that is gone.
struct UnderWay<'a>(&'a FrontsAhead);

impl Drop for UnderWay<'_> {
    fn drop(&mut self) {
        self.0.in_flight.send_modify(|count| *count -= 1);
    }
}

/// Reads the fronts of the Containers `folder`'s Entries live in, in the
/// background, and returns at once.
///
/// See [`OpenLibrary::read_fronts_of`](coffret_device::OpenLibrary::read_fronts_of)
/// for which ones. A Library locked by the time the task runs reads nothing:
/// the memory the fronts would go into went with the keys.
pub(crate) fn read_fronts_ahead(state: Arc<ServerState>, folder: Option<EntryPath>) {
    state.fronts.in_flight.send_modify(|count| *count += 1);
    tokio::spawn(async move {
        // Taken over from the count above, so it is let go wherever this ends.
        let _under_way = UnderWay(&state.fronts);
        let Ok(library) = state.unlocked() else {
            return;
        };
        if let Err(error) = library.read_fronts_of(folder.as_ref()).await {
            warn!(
                operation = "read_fronts",
                error = %error.redacted(),
                "the fronts of a listed folder's Containers were not read ahead",
            );
        }
    });
}
