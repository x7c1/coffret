use std::path::Path;

use coffret_model::Mtime;

/// How a backend arranges the folder its [`MappedRoots`](crate::MappedRoots)
/// then reads.
///
/// The cases are about what the capability *answers*, so the arranging cannot go
/// through the capability itself: a suite that wrote its files with the thing
/// under test would prove only that it agrees with itself. It is the backend's
/// instead — real files under a temporary directory for the gateway, entries in
/// a map for the fake — and this is the whole of what a case needs to say.
///
/// The calls are ordinary and not async: what they stand for is a person putting
/// files somewhere before a run starts, and neither backend needs a runtime for
/// it.
///
/// Each of them panics rather than answering with a failure: an arrangement that
/// will not go is a broken fixture, and a case that carried on from one would be
/// asserting about a folder nobody made.
pub trait FolderArrangement: Send + Sync {
    /// Makes a folder, and the folders above it.
    fn create_dir(&self, path: &Path);

    /// Puts a file at `path` with exactly these bytes and this modification
    /// time, making the folders above it.
    ///
    /// The time is the case's rather than the clock's, because what a listing
    /// reports about it is one of the things being asserted (spec: FM-9).
    fn write_file(&self, path: &Path, bytes: &[u8], mtime: Mtime);

    /// Atomically replaces the name with a new regular file.
    fn replace_file(&self, path: &Path, bytes: &[u8], mtime: Mtime);

    /// Puts something at `path` that is neither a file nor a folder.
    ///
    /// A symbolic link on a real filesystem, which is the shape EP-8 is actually
    /// about; a planted "other" in a fake, which has no links to make.
    fn plant_other(&self, path: &Path);

    /// Removes a folder and everything under it.
    fn remove_dir_all(&self, path: &Path);
}
