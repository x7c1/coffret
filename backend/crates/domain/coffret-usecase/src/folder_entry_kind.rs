use coffret_model::{Btime, Mtime};

/// What one name in a mapped folder turned out to stand for, with links
/// unfollowed (spec: EP-8).
///
/// Three answers and not the filesystem's whole vocabulary, because three is
/// what the walk above decides on: a folder to descend into, a regular file to
/// carry into the Library, and everything else to leave alone. A symbolic link
/// is the "everything else" the rule is actually about — it is never followed
/// and never given an Entry Path of its own — and a device node or a socket
/// under a mapped folder is the same non-answer for the same reason.
///
/// The times ride along with the file because this is the one call that read
/// them: a second stat would answer about a file that may already have moved,
/// and a birth time cannot be recovered at all once the local file is gone
/// (spec: FM-9).
#[derive(Debug, Clone)]
pub enum FolderEntryKind {
    /// A regular file, with what the filesystem said about it.
    File {
        /// Its length in bytes.
        size: u64,
        /// When it was last modified, which is the value an Entry carries
        /// (spec: FM-9).
        mtime: Mtime,
        /// When it came into being, where the platform reports it
        /// (spec: FM-9).
        btime: Option<Btime>,
    },
    /// A directory, which the walk descends into.
    Folder,
    /// A symbolic link, or anything else that is neither of the two.
    Other,
}
