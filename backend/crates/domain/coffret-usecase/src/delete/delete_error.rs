use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use coffret_model::{ContainerId, Redacted};

use crate::commit::{CommitError, CommitFailure};
use crate::error::Error;
use crate::index_error::IndexError;
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;
use crate::read_modify_replace::RebuildError;
use crate::upload::UploadError;

/// Result alias for a deletion.
pub type DeleteResult<T> = std::result::Result<T, DeleteError>;

/// Everything a deletion can fail with.
///
/// What it does not hold is a Container that would not verify, or one whose key
/// is lost: those cost the one Container, and are reported on
/// [`DeleteOutcome::refused`](super::DeleteOutcome::refused) while the rest of
/// the deletion commits (spec: PK-10). What is here stops the whole run, and
/// nothing is committed: before the Journal record exists the batch has
/// changed nothing (spec: CP-1).
///
/// The variants that wrap a layer below say in their own message only which
/// layer the run was in, and leave what that layer reported to the cause they
/// hand on — the reading every flow's error in this crate gives.
///
/// There is deliberately no `PartialEq`: a caller decides from the variant and
/// the fields it names, never by comparing two errors.
#[derive(Debug)]
pub enum DeleteError {
    /// Storage failed, or answered something the run cannot go on from.
    Storage(Error),
    /// The Index could not be read or written.
    Index(IndexError),
    /// A Container could not be encoded, or a key could not be drawn or
    /// wrapped.
    Format(coffret_format::Error),
    /// The run entered the commit flow and did not come through it.
    ///
    /// Including the catch-up and the read of the committed Keyring before the
    /// plan, which are the commit flow's routines (spec: CK-9, KL-1). Where the
    /// run reached its commit, the failure carries every Keyring repair the
    /// commit performed before it failed (spec: KL-15).
    Commit(CommitFailure),
    /// A spool file could not be created, written, flushed, read, or removed.
    ///
    /// The path is in the value and not in the message: a local path may never
    /// reach a diagnostic event, and a message is what is most likely logged.
    Io {
        /// What the run was doing.
        operation: LocalOperation,
        /// The file or directory it was doing it to.
        path: PathBuf,
        /// What the operating system reported.
        cause: io::Error,
    },
    /// The provider's digest of a rebuilt Pack it stored is not the digest of
    /// the bytes that were sent.
    ///
    /// The run stops with the Journal untouched and the object named by no
    /// record — an uncommitted Container this device's own pending row still
    /// accounts for (spec: CP-1, OC-2).
    TransferCorrupted {
        /// The Container whose object did not arrive whole.
        container_id: ContainerId,
        /// The digest taken while the spool was written.
        expected: String,
        /// The digest the provider reports for what it stored.
        actual: String,
    },
    /// The committed Keyring says nothing at all about a current Container the
    /// deletion has to rebuild.
    ///
    /// Neither an envelope nor a key-lost marker, which KL-7 does not admit for
    /// a current Container at a commit boundary — so it is the catalog and the
    /// Keyring disagreeing about what is current, and not a loss to report
    /// (spec: KL-7).
    UnmappedContainer {
        /// The Container the key table does not mention.
        container_id: ContainerId,
    },
}

impl fmt::Display for DeleteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(_) => f.write_str("the deletion did not get what it asked of Storage"),
            Self::Index(_) => f.write_str("the deletion could not read or write the Index"),
            Self::Format(_) => {
                f.write_str("the deletion could not encode a Container or draw or wrap a key")
            }
            Self::Commit(_) => f.write_str("the deletion did not come through the commit flow"),
            Self::Io { operation, .. } => {
                write!(f, "a spool file or folder could not be {operation}")
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
            Self::UnmappedContainer { container_id } => write!(
                f,
                "the committed Keyring maps Container {container_id} to neither an envelope nor \
                 a key-lost marker"
            ),
        }
    }
}

impl error::Error for DeleteError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            Self::Index(error) => Some(error),
            Self::Format(error) => Some(error),
            Self::Commit(error) => Some(error),
            Self::Io { cause, .. } => Some(cause),
            Self::TransferCorrupted { .. } | Self::UnmappedContainer { .. } => None,
        }
    }
}

impl Redacted for DeleteError {
    /// No local name of anything, and no Entry Path: the shape of the failure
    /// kept whole.
    fn redacted(&self) -> String {
        match self {
            Self::Storage(error) => format!("Delete::Storage: {}", error.redacted()),
            Self::Index(error) => format!("Delete::Index: {}", error.redacted()),
            Self::Format(error) => format!("Delete::Format: {}", error.redacted()),
            Self::Commit(error) => format!("Delete::Commit: {}", error.redacted()),
            Self::Io {
                operation, cause, ..
            } => format!("Delete::Io(operation={operation}, kind={:?})", cause.kind()),
            Self::TransferCorrupted { container_id, .. } => {
                format!("Delete::TransferCorrupted(container={container_id})")
            }
            Self::UnmappedContainer { container_id } => {
                format!("Delete::UnmappedContainer(container={container_id})")
            }
        }
    }
}

impl From<Error> for DeleteError {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}

impl From<IndexError> for DeleteError {
    fn from(error: IndexError) -> Self {
        Self::Index(error)
    }
}

impl From<coffret_format::Error> for DeleteError {
    fn from(error: coffret_format::Error) -> Self {
        Self::Format(error)
    }
}

impl From<CommitError> for DeleteError {
    fn from(error: CommitError) -> Self {
        Self::Commit(error.into())
    }
}

impl From<CommitFailure> for DeleteError {
    fn from(failure: CommitFailure) -> Self {
        Self::Commit(failure)
    }
}

impl From<LocalIoError> for DeleteError {
    fn from(error: LocalIoError) -> Self {
        Self::Io {
            operation: error.operation,
            path: error.path,
            cause: error.cause,
        }
    }
}

impl From<RebuildError> for DeleteError {
    fn from(error: RebuildError) -> Self {
        match error {
            RebuildError::Storage(error) => Self::Storage(error),
            RebuildError::Index(error) => Self::Index(error),
            RebuildError::Local(error) => Self::from(error),
            RebuildError::Format(error) => Self::Format(error),
        }
    }
}

impl From<UploadError> for DeleteError {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_paths::entry_path;

    /// The links a caller printing `{error:#}` reads, outermost first.
    fn chain(error: &dyn error::Error) -> Vec<String> {
        let mut links = vec![error.to_string()];
        let mut below = error.source();
        while let Some(link) = below {
            links.push(link.to_string());
            below = link.source();
        }
        links
    }

    // A wrapper says which layer, the cause says what that layer answered, and
    // the chain a caller prints holds each of those once.
    #[test]
    fn a_refused_commit_reaches_a_caller_as_two_different_sentences() {
        let error = DeleteError::Commit(
            CommitError::EntryPathCollision {
                path: entry_path("albums/spring.jpg"),
            }
            .into(),
        );

        assert_eq!(
            chain(&error),
            vec![
                "the deletion did not come through the commit flow".to_owned(),
                "two current Entries would claim the Entry Path \"albums/spring.jpg\"".to_owned(),
            ],
        );
        assert_eq!(
            error.redacted(),
            "Delete::Commit: Commit::EntryPathCollision(path_len=17)",
        );
    }

    // A spool path stays in the value: the message names the operation and the
    // redacted rendering the kind of refusal, and neither the path.
    #[test]
    fn a_refused_spool_names_no_path() {
        let error = DeleteError::Io {
            operation: LocalOperation::Flushing,
            path: PathBuf::from("/home/someone/spool/a-container.spool"),
            cause: io::Error::from(io::ErrorKind::PermissionDenied),
        };

        assert_eq!(
            chain(&error),
            vec![
                "a spool file or folder could not be flushed".to_owned(),
                "permission denied".to_owned(),
            ],
        );
        assert!(!error.redacted().contains("someone"));
    }
}
