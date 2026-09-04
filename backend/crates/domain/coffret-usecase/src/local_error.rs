use std::io;
use std::path::PathBuf;

use coffret_model::EntryPath;

use crate::local_operation::LocalOperation;

/// What the steps that touch this device's own disk can fail with.
///
/// Walking the mapped folders and writing a spool are the same work whichever
/// flow asks for it, so they are written once and reported in a vocabulary of
/// their own. Each flow's public error type carries these verdicts under its own
/// names — a caller of [`sync`](crate::sync) still matches
/// [`SyncError`](crate::sync::SyncError) — which is what lets the steps be
/// shared without the two flows sharing one error enum.
#[derive(Debug)]
pub(crate) enum LocalError {
    /// A local file could not be walked, read, written, or removed.
    ///
    /// The path is in the value and not in the message: a local path is one of
    /// the things that may never reach a log line, and an error's message is the
    /// part most likely to be logged verbatim.
    Io {
        /// What the run was doing.
        operation: LocalOperation,
        /// The file or directory it was doing it to.
        path: PathBuf,
        /// What the operating system reported.
        cause: io::Error,
    },
    /// A local filename is not a name the Library can hold, so it spells no
    /// Entry Path (spec: EP-1, EP-2).
    ///
    /// A name that is not valid Unicode is the one this is nearly always about.
    /// A name that is valid Unicode and still not a path component — which
    /// nothing a directory listing returns is — is the same fact about the same
    /// file, and is reported the same way rather than through a second refusal
    /// nobody could tell apart.
    UnrepresentableName {
        /// The file whose name the Library has no position for.
        path: PathBuf,
    },
    /// Two local files under the device's mappings claim one Entry Path
    /// (spec: EP-4, EP-5).
    PathCollision {
        /// The path claimed twice.
        path: EntryPath,
    },
}

impl LocalError {
    /// A failed filesystem operation, in this vocabulary.
    pub(crate) fn io(
        operation: LocalOperation,
        path: impl Into<PathBuf>,
        cause: io::Error,
    ) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            cause,
        }
    }
}
