use std::os::fd::OwnedFd;
use std::path::PathBuf;

use coffret_usecase::{DescentError, LocalOperation};
use rustix::io::Errno;

use crate::unix_destinations::refusal;

/// The folder one file belongs in, held open by the descent that reached it.
///
/// A file descriptor and not a path, which is the whole of the confinement: a
/// path rebuilt from the mapped root and handed back to the operating system
/// would ask "which folder is this" a second time, and a name that became a
/// symbolic link in between would be followed on that second asking
/// (spec: EP-4, EP-11).
///
/// It also carries the file's own name — the last of the components the descent
/// was given — because that is what the rename publishes onto, and the folder's
/// path, which is only ever used to say which file a refusal is about.
///
/// The three steps of one placement share it behind an [`Arc`](std::sync::Arc):
/// the destination the caller keeps, the scratch file it opened, and the flushed
/// file that renames it are three handles on one open folder, and the folder has
/// to outlive all three.
pub(super) struct OpenFolder {
    /// The open folder every call is made against.
    directory: OwnedFd,
    /// Its path, for saying which folder a refusal is about — never for
    /// reaching it again.
    folder: PathBuf,
    /// What the file itself is called inside that folder.
    name: String,
}

impl OpenFolder {
    /// The folder at `folder`, open as `directory`, holding a file called
    /// `name`.
    pub(super) fn new(directory: OwnedFd, folder: PathBuf, name: String) -> Self {
        Self {
            directory,
            folder,
            name,
        }
    }

    /// The open folder itself, for the `*at` calls that name a file inside it.
    pub(super) fn directory(&self) -> &OwnedFd {
        &self.directory
    }

    /// What the file itself is called.
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    /// Where one of this folder's files stands, for a message to name.
    ///
    /// Nothing reaches the filesystem through it: every call that does is made
    /// against the open folder, by name.
    pub(super) fn path_of(&self, name: &str) -> PathBuf {
        self.folder.join(name)
    }

    /// What the operating system refused about one name in this folder.
    pub(super) fn refused(
        &self,
        name: &str,
        operation: LocalOperation,
        cause: Errno,
    ) -> DescentError {
        refusal(&self.path_of(name), operation, cause)
    }
}
