use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use coffret_model::{ContainerId, EntryPath, Redacted};

use crate::commit::CommitError;
use crate::error::Error;
use crate::index_error::IndexError;
use crate::local_error::LocalError;
use crate::local_operation::LocalOperation;
use crate::upload::UploadError;

/// Result alias for the folder sync.
pub type SyncResult<T> = std::result::Result<T, SyncError>;

/// Everything a folder sync can fail with.
///
/// A vocabulary of its own, for the reason [`CommitError`] is one: the sync
/// fails at things no port ever reports — a local filename that is not a
/// spelling of any Entry Path, two files that would claim one Entry Path, an
/// object that did not arrive as it left — and folding those into a port's
/// error type would make that type answer questions it was never asked.
///
/// What the layers below report travels unchanged inside [`SyncError::Storage`],
/// [`SyncError::Index`], [`SyncError::Format`], and [`SyncError::Commit`]. The
/// commit's own vocabulary is wrapped rather than flattened: whether a batch was
/// refused for an Entry Path collision or rebased past its attempt limit is a
/// distinction the commit flow already draws, and re-drawing it here would give
/// one verdict two spellings.
///
/// There is deliberately no `PartialEq`: a caller decides from the variant and
/// the fields it names, never by comparing two errors.
#[derive(Debug)]
pub enum SyncError {
    /// Storage failed, or answered something the run cannot go on from.
    Storage(Error),
    /// The Index could not be read or written.
    Index(IndexError),
    /// A Container could not be encoded, or a key could not be wrapped.
    Format(coffret_format::Error),
    /// A run entered the commit flow and did not come through it.
    ///
    /// Usually with a batch. But the catch-up a run makes before it reads its
    /// catalog at all — to scan against the Library as it stands, and to settle
    /// the pending rows an interrupted run left against it — is the commit
    /// flow's routine and fails in its vocabulary, so this also reaches a caller
    /// whose run had no batch and never reached one (spec: CK-9, OC-3, OC-7).
    Commit(CommitError),
    /// A local file could not be walked, read, written, or removed.
    ///
    /// The path is in the value and not in the message, for the reason
    /// [`UnrepresentablePath`](crate::IndexError::UnrepresentablePath) keeps
    /// one there: a local path is one of the things that may never reach a log
    /// line, and an error's message is the part most likely to be logged
    /// verbatim.
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
    /// The provider's digest of a Container it stored is not the digest of the
    /// bytes that were sent.
    ///
    /// The object reached Storage corrupted, or not all of it did. Either way
    /// it is not the Container the batch would commit, so the run stops with
    /// the Journal untouched and the object un-named by any record — an
    /// uncommitted Container this device's own pending row still accounts for
    /// (spec: CP-1, OC-2).
    TransferCorrupted {
        /// The Container whose object did not arrive whole.
        container_id: ContainerId,
        /// The digest taken while the spool was written.
        expected: String,
        /// The digest the provider reports for what it stored.
        actual: String,
    },
    /// Storage handed back listing pages without ever reaching the last one.
    ///
    /// The walk that asks Storage what it stored is bounded, because a provider
    /// that keeps answering with another continuation token would otherwise
    /// keep a run going forever. Reaching that bound is this flow's own verdict
    /// and not something Storage reported, which is why it is a variant here
    /// rather than a [`SyncError::Storage`] the run made up on the provider's
    /// behalf.
    ListingLimitReached {
        /// How many pages were taken before the run stopped asking.
        pages: usize,
    },
}

impl fmt::Display for SyncError {
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
            } => {
                write!(
                    f,
                    "a local file or folder could not be {operation}: {cause}"
                )
            }
            Self::UnrepresentableName { .. } => {
                f.write_str("a local filename is not valid Unicode, so it spells no Entry Path")
            }
            // The Entry Path is what identifies the collision, so the message
            // carries it — which is why a log line renders this through
            // [`Redacted`] instead: an Entry Path never belongs in one.
            Self::PathCollision { path } => write!(
                f,
                "two local files would claim the Entry Path {:?}",
                path.as_str()
            ),
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

impl error::Error for SyncError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            Self::Index(error) => Some(error),
            Self::Format(error) => Some(error),
            Self::Commit(error) => Some(error),
            Self::Io { cause, .. } => Some(cause),
            Self::UnrepresentableName { .. }
            | Self::PathCollision { .. }
            | Self::TransferCorrupted { .. }
            | Self::ListingLimitReached { .. } => None,
        }
    }
}

impl Redacted for SyncError {
    /// Which refusal it is, and what a walk of the mapped folders may say about
    /// itself.
    ///
    /// Every local name the walk met is out, whether it is an Entry Path the
    /// Library holds or a filename on this device that spells none: both are
    /// the user's own names for their own files (spec: EP-1). What is left is
    /// the shape of the failure and the counts and identifiers around it, which
    /// is what a reader asking "did this run get anywhere" is after.
    fn redacted(&self) -> String {
        match self {
            Self::Storage(error) => format!("Sync::Storage: {}", error.redacted()),
            Self::Index(error) => format!("Sync::Index: {}", error.redacted()),
            Self::Format(error) => format!("Sync::Format: {}", error.redacted()),
            Self::Commit(error) => format!("Sync::Commit: {}", error.redacted()),
            Self::Io {
                operation, cause, ..
            } => format!("Sync::Io(operation={operation}, kind={:?})", cause.kind()),
            Self::UnrepresentableName { .. } => "Sync::UnrepresentableName".to_owned(),
            Self::PathCollision { path } => {
                format!("Sync::PathCollision(path_len={})", path.as_str().len())
            }
            Self::TransferCorrupted { container_id, .. } => {
                format!("Sync::TransferCorrupted(container={container_id})")
            }
            Self::ListingLimitReached { pages } => {
                format!("Sync::ListingLimitReached(pages={pages})")
            }
        }
    }
}

impl From<Error> for SyncError {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}

impl From<IndexError> for SyncError {
    fn from(error: IndexError) -> Self {
        Self::Index(error)
    }
}

impl From<coffret_format::Error> for SyncError {
    fn from(error: coffret_format::Error) -> Self {
        Self::Format(error)
    }
}

impl From<CommitError> for SyncError {
    fn from(error: CommitError) -> Self {
        Self::Commit(error)
    }
}

impl From<LocalError> for SyncError {
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

impl From<UploadError> for SyncError {
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

#[cfg(test)]
mod tests {
    use super::*;

    // EP-4: the message names the path because that is what a person has to go
    // and look at; the log line says only that two files claimed one.
    #[test]
    fn two_files_claiming_one_path_are_recorded_without_it() {
        let error = SyncError::PathCollision {
            path: EntryPath::nfc("albums/spring.jpg"),
        };

        assert!(error.to_string().contains("albums/spring.jpg"));
        assert_eq!(error.redacted(), "Sync::PathCollision(path_len=17)");
    }
}
