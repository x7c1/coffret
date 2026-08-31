use coffret_device::{EntryFetches, OpenLibrary};

use crate::fill::Fills;
use crate::sync::Syncs;

/// One open Library, and what serving it needs beyond it.
///
/// The Passphrase was spent once, at startup, and what is here is what it
/// produced: one process is one unlock, and the derived keys live as long as the
/// process (spec: DK-9). Nothing in this value ever leaves it — no key, no
/// ciphertext, no token reaches a response — and what a browser is answered with
/// is drawn from it a request at a time.
pub struct ServerState {
    /// What this device calls the Library, which is what was typed to start the
    /// server and what the status bar shows.
    ///
    /// It is this device's name for it rather than the Library's own: another
    /// device holding the same Library may call it something else (spec: CK-7).
    pub name: String,
    /// The Library, open.
    pub library: OpenLibrary,
    /// Who is already fetching which Entry, so two readers wanting one page
    /// fetch it once.
    pub fetches: EntryFetches,
    /// Which folder is being brought over in the background, and how far it has
    /// got.
    ///
    /// State of this process rather than of the Library, exactly as
    /// [`fetches`](Self::fetches) is: it is about work in flight here, it is
    /// gone when the process is, and nothing in it is ever uploaded.
    pub fills: Fills,
    /// Whether the mapped folders are being carried into the Library right now,
    /// and what the last run of that came to.
    ///
    /// The other half of [`fills`](Self::fills), going the other way, and device
    /// state in exactly the same sense. The two are separate because they are
    /// separate work over one Library and neither waits on the other: a folder
    /// being brought over and a dropped file being carried in can be happening at
    /// once, and a browser is told about both.
    pub syncs: Syncs,
}

impl ServerState {
    /// Serves the Library that was opened, under the name it was opened by.
    pub fn new(name: String, library: OpenLibrary) -> Self {
        Self {
            name,
            library,
            fetches: EntryFetches::new(),
            fills: Fills::new(),
            syncs: Syncs::new(),
        }
    }
}
