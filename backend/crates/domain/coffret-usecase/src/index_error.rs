use std::error;
use std::fmt;
use std::path::PathBuf;

use coffret_model::{ContainerId, EntryPath};

/// Result alias for [`Index`](crate::Index) operations.
pub type IndexResult<T> = std::result::Result<T, IndexError>;

/// Everything an [`Index`](crate::Index) operation can fail with.
///
/// It is a vocabulary of its own rather than the Storage port's
/// [`Error`](crate::Error), because the two ports fail at different things: the
/// Index is a device-local catalog that no provider is involved in, so nothing
/// here is a lost race, a throttle, or a transport fault, and nothing here is
/// worth retrying unchanged.
///
/// The catalog being a cache and never the source of truth (spec: RV-5) is what
/// makes the failures here small: whatever cannot be read back can be rebuilt
/// from Storage, so the type says what the caller must do — rebuild, resolve a
/// conflict, install a build that knows the file, or name the local file it
/// cannot keep — rather than describing a backend.
#[derive(Debug)]
pub enum IndexError {
    /// The Index has never been given a committed state to stand on.
    ///
    /// A fresh Index catalogs nothing and checkpoints nothing until a
    /// [`restore`](crate::Index::restore) adopts a Snapshot or an
    /// [`apply`](crate::Index::apply) replays a record, so there is no content
    /// to hand back and no checkpoint to write into one (spec: CK-9).
    NoCheckpoint,
    /// Two Entries would occupy one Entry Path.
    ///
    /// At every committed Library state one Entry Path identifies at most one
    /// current Entry, so a replay or a restore that would place a second one
    /// there is describing a state no commit could have produced (spec: EP-5,
    /// EP-6).
    DuplicatePath {
        /// The path claimed twice.
        path: EntryPath,
    },
    /// One Container would be added to the current set twice.
    DuplicateContainer {
        /// The Container claimed twice.
        container_id: ContainerId,
    },
    /// An Entry names a Container the current set does not hold.
    ///
    /// A record carries the Entries of the Containers it adds, so an Entry
    /// without its Container is a record or a Snapshot that cannot be replayed
    /// as it stands (spec: CP-11).
    UnknownContainer {
        /// The Container the Entry names.
        container_id: ContainerId,
    },
    /// A local path the catalog has no way to keep.
    ///
    /// A filesystem may hand out a name that is not valid UTF-8, and a catalog
    /// that keeps paths as text cannot store one without changing it. Keeping a
    /// lossy spelling would point a mapping or a spool at a file that is not the
    /// one meant, so the path is refused instead.
    ///
    /// The path travels in the value so a caller can say which file it is
    /// about; the message leaves it out, because a local path is the user's own
    /// and a message may end up wherever an error is reported.
    UnrepresentablePath {
        /// What the Index was doing.
        operation: &'static str,
        /// The path that cannot be kept.
        path: PathBuf,
    },
    /// The catalog was written by a build that laid it out differently.
    ///
    /// The Index is a cache, so the answer is to discard the file and rebuild
    /// from Storage rather than to guess at the layout (spec: RV-5).
    UnsupportedSchema {
        /// The version found in the file.
        found: i64,
        /// The version this build writes and reads.
        supported: i64,
    },
    /// The catalog holds a value this build cannot read back.
    ///
    /// A Container kind spelled in a vocabulary this build has no reading for,
    /// a stored digest the domain does not admit, half a reference where a
    /// whole one belongs: the file was written by something else, or damaged.
    /// The answer is the one [`IndexError::UnsupportedSchema`] asks for —
    /// discard the file and rebuild from Storage (spec: RV-5) — and not the one
    /// a store that merely failed asks for, which is why the two are separate.
    UnreadableCatalog {
        /// What the Index was doing.
        operation: &'static str,
        /// What could not be read, as whatever refused it reported.
        cause: Box<dyn error::Error + Send + Sync>,
    },
    /// The store the catalog is kept in failed.
    ///
    /// The store itself, not what it holds: a file that cannot be opened, a
    /// write that cannot be carried out, a thread that never finished.
    Backend {
        /// What the Index was doing.
        operation: &'static str,
        /// What the store reported.
        cause: Box<dyn error::Error + Send + Sync>,
    },
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCheckpoint => f.write_str("the Index stands at no committed Library state"),
            // The Entry Path is what identifies the conflict, so the message
            // carries it — and the Index therefore logs nothing, because an
            // Entry Path never belongs in a log line.
            Self::DuplicatePath { path } => {
                write!(f, "two Entries claim the Entry Path {path:?}")
            }
            Self::DuplicateContainer { container_id } => {
                write!(f, "Container {container_id} is added twice")
            }
            Self::UnknownContainer { container_id } => {
                write!(f, "no current Container {container_id} to hold this Entry")
            }
            // The path stays out of the message, and stays in the value: see
            // the variant.
            Self::UnrepresentablePath { operation, .. } => write!(
                f,
                "a local path given while {operation} is not one this catalog can keep: \
                 it is not valid UTF-8"
            ),
            Self::UnsupportedSchema { found, supported } => write!(
                f,
                "the Index file is at schema version {found}, this build reads {supported}"
            ),
            Self::UnreadableCatalog { operation, cause } => {
                write!(
                    f,
                    "the Index file holds something this build cannot read while {operation}: \
                     {cause}"
                )
            }
            Self::Backend { operation, cause } => {
                write!(f, "the Index store failed while {operation}: {cause}")
            }
        }
    }
}

impl error::Error for IndexError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::UnreadableCatalog { cause, .. } | Self::Backend { cause, .. } => {
                Some(cause.as_ref())
            }
            _ => None,
        }
    }
}
