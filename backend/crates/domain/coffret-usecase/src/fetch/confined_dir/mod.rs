use std::io;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use rustix::io::Errno;

use crate::fetch::descent_error::DescentError;
use crate::local_operation::LocalOperation;

mod create;

mod descent;
pub(super) use descent::{descend, look_up};

mod look;

mod publish;

mod remove;

/// The folder one file belongs in, held open, reached without passing through
/// anything but a real directory of the mapped root.
///
/// This is what "placing a file inside a mapped folder" means once EP-4 and
/// EP-11 are taken seriously. An Entry Path comes from another enrolled device
/// and says nothing about the shape of *this* device's disk: the same
/// `link/authorized_keys` is an ordinary folder and a file on the device that
/// committed it, and may be a symbolic link out of the mapped root here. A
/// writer that joined the components onto the root and handed the string to the
/// operating system would follow that link and write where the Library never
/// pointed. So the descent walks the components one at a time and refuses
/// anything that is not a real directory, and what it hands back is the open
/// directory rather than a path (spec: EP-4, EP-11).
///
/// Every write is then made *relative to this handle* — the temporary file, the
/// rename that publishes it, the removal that cleans it up — so no answer can go
/// stale between the descent and the write. A path rebuilt from the root and
/// handed back to the operating system would ask the question again, and a name
/// that became a symbolic link in between would be followed on that second
/// asking.
///
/// The mapped root itself is opened as the path the user configured it as. What
/// that path points at is theirs to choose (spec: EP-9); what is under it is the
/// Library's, and that is what is walked component by component.
///
/// Unix only, deliberately. The primitives are `openat`, `mkdirat`, `renameat`,
/// and `unlinkat` with `O_NOFOLLOW` and `O_DIRECTORY`, which is what expresses
/// "descend one name without following a link" — coffret runs on Linux and
/// macOS, and a portability layer over platforms it does not run on would be a
/// second answer to a question that has one.
pub struct ConfinedDir {
    /// The open folder every call below is made against.
    directory: OwnedFd,
    /// Its path, for saying which folder a refusal is about — never for
    /// reaching it again.
    folder: PathBuf,
    /// What the file itself is called inside that folder.
    name: String,
}

impl ConfinedDir {
    /// Where one of this folder's files stands, for an error to name.
    ///
    /// Nothing reaches the filesystem through it: every call that does is made
    /// against the open folder, by name.
    pub fn path_of(&self, name: &str) -> PathBuf {
        self.folder.join(name)
    }

    /// What the operating system refused about one name in this folder.
    fn refused(&self, name: &str, operation: LocalOperation, cause: Errno) -> DescentError {
        refusal(&self.folder.join(name), operation, cause)
    }
}

/// What one refused syscall means: a fence the descent met, or an I/O failure.
///
/// `ELOOP` is what `O_NOFOLLOW` reports for a symbolic link and `ENOTDIR` what
/// `O_DIRECTORY` reports for anything else that is not a folder; `EMLINK` is the
/// same verdict as `ELOOP` on the BSDs. All three say the path cannot be
/// materialized here rather than that the disk went wrong.
fn refusal(at: &Path, operation: LocalOperation, cause: Errno) -> DescentError {
    if cause == Errno::LOOP || cause == Errno::NOTDIR || cause == Errno::MLINK {
        return DescentError::Blocked {
            path: at.to_path_buf(),
        };
    }
    DescentError::Io {
        operation,
        path: at.to_path_buf(),
        cause: io::Error::from(cause),
    }
}
