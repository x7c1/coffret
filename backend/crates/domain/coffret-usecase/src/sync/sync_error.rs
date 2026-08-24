use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use coffret_model::{ContainerId, EntryPath};

use crate::commit::CommitError;
use crate::error::Error;
use crate::index_error::IndexError;

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
    /// The batch reached the commit flow and did not come through it.
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

/// What a local file or folder was being asked for when the operating system
/// refused.
///
/// A value rather than the word that goes in the message: a caller telling
/// somebody what to go and fix has a different sentence for a spool it could
/// not write than for a source file it could not read, and finding out which
/// happened by matching on prose is not something a public error type may ask
/// of it. The word itself is this type's [`Display`](fmt::Display), which is
/// where a rendering belongs.
///
/// There is deliberately no `PartialEq`, for the reason [`SyncError`] has none.
#[derive(Debug, Clone, Copy)]
pub enum LocalOperation {
    /// A directory's entries were being read.
    Listing,
    /// A directory entry's own metadata was being read, links unfollowed
    /// (spec: EP-8).
    Stating,
    /// A source file's plaintext was being read.
    Reading,
    /// A spool file or the spool directory was being made.
    Creating,
    /// Ciphertext was going into a spool file.
    Writing,
    /// A spool file was being flushed to the device, so that it outlasts the
    /// run that wrote it (spec: OC-2).
    Flushing,
    /// A spool file whose Container was committed or abandoned was being
    /// deleted (spec: OC-6).
    Removing,
}

impl fmt::Display for LocalOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Listing => "listed",
            Self::Stating => "stated",
            Self::Reading => "read",
            Self::Creating => "created",
            Self::Writing => "written",
            Self::Flushing => "flushed",
            Self::Removing => "removed",
        })
    }
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
            // carries it — and the sync therefore logs nothing about it,
            // because an Entry Path never belongs in a log line.
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
