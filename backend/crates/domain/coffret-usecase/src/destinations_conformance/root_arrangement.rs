use std::path::Path;

use coffret_model::Mtime;

/// How a backend arranges the mapped root its
/// [`Destinations`](crate::Destinations) then writes into, and reads it back.
///
/// The cases are about what the capability *answers*, so neither the arranging
/// nor the reading back can go through the capability itself: a suite that
/// planted its files with the thing under test would prove only that it agrees
/// with itself. It is the backend's instead — real files under a temporary
/// directory for the gateway, entries in a map for the fake — and this is the
/// whole of what a case needs to say.
///
/// The calls are ordinary and not async: what they stand for is a person putting
/// files somewhere before a run starts, or a person looking at the folder
/// afterwards, and neither backend needs a runtime for either.
///
/// Each of the arranging calls panics rather than answering with a failure: an
/// arrangement that will not go is a broken fixture, and a case that carried on
/// from one would be asserting about a folder nobody made.
pub trait RootArrangement: Send + Sync {
    /// Puts a file at `path` with exactly these bytes and this modification
    /// time, making the folders above it.
    ///
    /// The time is the case's rather than the clock's, because what a look
    /// reports about it is one of the things being asserted (spec: FM-9).
    fn write_file(&self, path: &Path, bytes: &[u8], mtime: Mtime);

    /// Puts something at `path` that is neither a file nor a folder.
    ///
    /// A symbolic link on a real filesystem, which is the shape EP-4 is actually
    /// about; a planted marker in a fake, which has no links to make.
    fn plant_other(&self, path: &Path);

    /// One file's whole content, or `None` where nothing is at that path.
    ///
    /// Read without following a link, so a name a case did not expect to be a
    /// file answers `None` rather than whatever a link points at (spec: EP-8).
    fn content(&self, path: &Path) -> Option<Vec<u8>>;

    /// One file's modification time, or `None` where nothing is at that path
    /// (spec: FM-9).
    fn mtime(&self, path: &Path) -> Option<Mtime>;

    /// Whether anything at all stands at `path` — a file, a folder, or whatever
    /// [`plant_other`](Self::plant_other) plants.
    ///
    /// Deliberately not saying which: a scratch name a case expects to be gone
    /// is gone whichever of the three would otherwise be standing there.
    fn holds(&self, path: &Path) -> bool;
}
