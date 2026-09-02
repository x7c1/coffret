use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use crate::local_operation::LocalOperation;

/// Why a writer could not reach the folder one file belongs in.
///
/// The vocabulary of the descent alone, so that the two writers that share it
/// can each say what a refusal means in their own words. A folder fetch meets
/// [`Blocked`](Self::Blocked) while deciding where it may write and reports that
/// Entry as
/// [`Surfaced::UnreachablePlace`](super::Surfaced::UnreachablePlace), placing the
/// rest; where there is no run to go on with — a placement whose folder changed
/// shape after the selection, and the explorer taking a dropped file into a
/// mapped folder — the same fence is
/// [`FetchError::UnmaterializablePath`](super::FetchError::UnmaterializablePath).
/// Neither invents a second spelling for what the descent found (spec: EP-4).
///
/// There is deliberately no `PartialEq`, for the reason the error types around
/// it have none: a caller decides from the variant and the fields it names.
#[derive(Debug)]
pub enum DescentError {
    /// Something on the way down is not a folder inside the mapped root.
    ///
    /// A symbolic link, an ordinary file where a folder must be, or a name that
    /// became one of those while the descent was walking past it. Any of them
    /// means the Entry Path cannot be materialized *here*: following it would
    /// put bytes somewhere the mapped root does not stand for, which is the one
    /// thing a device may never do with a path another device committed
    /// (spec: EP-4, EP-11). The scan side refuses the mirror of this by not
    /// following links out of a mapped folder (spec: EP-8).
    Blocked {
        /// The component the descent stopped at.
        ///
        /// In the value and not in the message, for the reason
        /// [`Io`](Self::Io) keeps one there: a local path is one of the things
        /// that may never reach a log line (spec: EP-1).
        path: PathBuf,
    },
    /// A folder on the way down, or the file itself, could not be made, read,
    /// renamed, or removed.
    Io {
        /// What the writer was doing.
        operation: LocalOperation,
        /// The file or folder it was doing it to.
        path: PathBuf,
        /// What the operating system reported.
        cause: io::Error,
    },
}

impl fmt::Display for DescentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blocked { .. } => f.write_str(
                "a folder on the way to a file is not one inside the mapped root, \
                 so no file here can stand for the Entry Path",
            ),
            Self::Io {
                operation, cause, ..
            } => write!(
                f,
                "a local file or folder could not be {operation}: {cause}"
            ),
        }
    }
}

impl error::Error for DescentError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Blocked { .. } => None,
            Self::Io { cause, .. } => Some(cause),
        }
    }
}
