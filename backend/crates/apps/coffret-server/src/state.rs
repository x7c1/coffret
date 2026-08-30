use coffret_device::{EntryFetches, OpenLibrary};

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
}

impl ServerState {
    /// Serves the Library that was opened, under the name it was opened by.
    pub fn new(name: String, library: OpenLibrary) -> Self {
        Self {
            name,
            library,
            fetches: EntryFetches::new(),
        }
    }
}
