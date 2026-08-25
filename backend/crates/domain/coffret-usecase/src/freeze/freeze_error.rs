use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use coffret_model::{ContainerId, EntryPath};

use crate::commit::CommitError;
use crate::error::Error;
use crate::index_error::IndexError;
use crate::local_error::LocalError;
use crate::local_operation::LocalOperation;
use crate::upload::UploadError;

/// Result alias for the freeze.
pub type FreezeResult<T> = std::result::Result<T, FreezeError>;

/// Everything a freeze can fail with.
///
/// A vocabulary of its own, for the reason [`SyncError`](crate::sync::SyncError)
/// is one. Most of what can go wrong is shared with the sync — the same walk of
/// the same folders, the same spool directory, the same upload — and is reported
/// here under the same names, because a caller of one flow should not have to
/// learn the other's spelling to read a failure.
///
/// What is only here is what only a Pack can hit: a local file that stopped
/// being the file the scan measured while the Pack around it was being written.
///
/// There is deliberately no `PartialEq`: a caller decides from the variant and
/// the fields it names, never by comparing two errors.
#[derive(Debug)]
pub enum FreezeError {
    /// Storage failed, or answered something the run cannot go on from.
    Storage(Error),
    /// The Index could not be read or written.
    Index(IndexError),
    /// A Container could not be encoded, or a key could not be wrapped or
    /// unwrapped.
    Format(coffret_format::Error),
    /// A run entered the commit flow and did not come through it.
    ///
    /// Usually with a batch. But the catch-up a run makes before its scan, and
    /// the read of the committed Keyring that tells it which Containers are
    /// unreadable, are both the commit flow's routines and fail in its
    /// vocabulary — so this also reaches a caller whose run had no batch and
    /// never reached one (spec: CK-9, KL-1).
    Commit(CommitError),
    /// A local file could not be walked, read, written, or removed.
    ///
    /// The path is in the value and not in the message, for the reason
    /// [`UnrepresentablePath`](crate::IndexError::UnrepresentablePath) keeps one
    /// there: a local path is one of the things that may never reach a log line,
    /// and an error's message is the part most likely to be logged verbatim.
    Io {
        /// What the run was doing.
        operation: LocalOperation,
        /// The file or directory it was doing it to.
        path: PathBuf,
        /// What the operating system reported.
        cause: io::Error,
    },
    /// A local filename is not valid Unicode, so it spells no Entry Path.
    ///
    /// Reported rather than skipped or renamed: a file coffret cannot name is a
    /// file it would silently fail to back up (spec: EP-1).
    UnrepresentableName {
        /// The file whose name could not be read as UTF-8.
        path: PathBuf,
    },
    /// Two local files under the device's mappings claim one Entry Path.
    ///
    /// Neither is selected and nothing is renamed: one Entry Path identifies at
    /// most one Entry, and choosing between two files claiming it is the user's
    /// (spec: EP-4, EP-5).
    PathCollision {
        /// The path claimed twice.
        path: EntryPath,
    },
    /// A file changed between being surveyed and being written into a Pack.
    ///
    /// A Pack's entry table is written before its content, because that is what
    /// lets the content stream (spec: PK-3). So a file whose length or content
    /// moved in between would land inside a Container whose table does not
    /// describe it, and the run stops instead — the Pack is abandoned in the
    /// spool, where this device's own pending row accounts for it (spec: OC-2).
    /// The file is simply eligible again next time.
    ///
    /// The Entry Path travels in the value rather than the message, for the
    /// reason the paths above do.
    SourceChanged {
        /// The Entry Path of the file that moved under the run.
        path: EntryPath,
    },
    /// The provider's digest of a Pack it stored is not the digest of the bytes
    /// that were sent.
    ///
    /// The object reached Storage corrupted, or not all of it did. Either way it
    /// is not the Container the batch would commit, so the run stops with the
    /// Journal untouched and the object un-named by any record — an uncommitted
    /// Container this device's own pending row still accounts for (spec: CP-1,
    /// OC-2).
    TransferCorrupted {
        /// The Container whose object did not arrive whole.
        container_id: ContainerId,
        /// The digest taken while the spool was written.
        expected: String,
        /// The digest the provider reports for what it stored.
        actual: String,
    },
    /// Storage handed back listing pages without ever reaching the last one.
    ListingLimitReached {
        /// How many pages were taken before the run stopped asking.
        pages: usize,
    },
}

impl fmt::Display for FreezeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(f, "{error}"),
            Self::Index(error) => write!(f, "{error}"),
            Self::Format(error) => write!(f, "{error}"),
            Self::Commit(error) => write!(f, "{error}"),
            // The path stays out of the message, and stays in the value: see
            // the variant.
            Self::Io {
                operation, cause, ..
            } => write!(
                f,
                "a local file or folder could not be {operation}: {cause}"
            ),
            Self::UnrepresentableName { .. } => {
                f.write_str("a local filename is not valid Unicode, so it spells no Entry Path")
            }
            Self::PathCollision { .. } => f.write_str("two local files would claim one Entry Path"),
            Self::SourceChanged { .. } => {
                f.write_str("a local file changed while the Pack holding it was being written")
            }
            Self::TransferCorrupted {
                container_id,
                expected,
                actual,
            } => write!(
                f,
                "Storage reports a digest of {actual} for Container {container_id}, \
                 and the bytes sent hash to {expected}"
            ),
            Self::ListingLimitReached { pages } => {
                write!(f, "a listing of Storage did not end within {pages} pages")
            }
        }
    }
}

impl error::Error for FreezeError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            Self::Index(error) => Some(error),
            Self::Format(error) => Some(error),
            Self::Commit(error) => Some(error),
            Self::Io { cause, .. } => Some(cause),
            Self::UnrepresentableName { .. }
            | Self::PathCollision { .. }
            | Self::SourceChanged { .. }
            | Self::TransferCorrupted { .. }
            | Self::ListingLimitReached { .. } => None,
        }
    }
}

impl From<Error> for FreezeError {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}

impl From<IndexError> for FreezeError {
    fn from(error: IndexError) -> Self {
        Self::Index(error)
    }
}

impl From<coffret_format::Error> for FreezeError {
    fn from(error: coffret_format::Error) -> Self {
        Self::Format(error)
    }
}

impl From<CommitError> for FreezeError {
    fn from(error: CommitError) -> Self {
        Self::Commit(error)
    }
}

impl From<LocalError> for FreezeError {
    /// What the shared walk and spool steps report, under this flow's names.
    fn from(error: LocalError) -> Self {
        match error {
            LocalError::Io {
                operation,
                path,
                cause,
            } => Self::Io {
                operation,
                path,
                cause,
            },
            LocalError::UnrepresentableName { path } => Self::UnrepresentableName { path },
            LocalError::PathCollision { path } => Self::PathCollision { path },
        }
    }
}

impl From<UploadError> for FreezeError {
    /// What the shared upload step reports, under this flow's names.
    fn from(error: UploadError) -> Self {
        match error {
            UploadError::Storage(error) => Self::Storage(error),
            UploadError::Index(error) => Self::Index(error),
            UploadError::TransferCorrupted {
                container_id,
                expected,
                actual,
            } => Self::TransferCorrupted {
                container_id,
                expected,
                actual,
            },
            UploadError::ListingLimitReached { pages } => Self::ListingLimitReached { pages },
        }
    }
}
