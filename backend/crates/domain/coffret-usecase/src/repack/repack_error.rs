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
use crate::pack_spool::PackSpoolError;
use crate::read_modify_replace::RebuildError;
use crate::repack::repack_refusal::RepackRefusal;
use crate::upload::UploadError;

/// Result alias for a repack.
pub type RepackResult<T> = std::result::Result<T, RepackError>;

/// Everything a repack can fail with.
///
/// Every one of them stops the run with nothing committed: before the Journal
/// record exists the batch has changed nothing (spec: CP-1). The one a person
/// acts on differently is [`Refused`](Self::Refused), a Pack that could not be
/// read; the rest are this device's or Storage's.
///
/// The variants that wrap a layer below say in their own message only which
/// layer the run was in, and leave what that layer reported to the cause they
/// hand on — the reading every flow's error in this crate gives.
///
/// There is deliberately no `PartialEq`: a caller decides from the variant and
/// the fields it names, never by comparing two errors.
#[derive(Debug)]
pub enum RepackError {
    /// Storage failed, or answered something the run cannot go on from.
    Storage(Error),
    /// The Index could not be read or written.
    Index(IndexError),
    /// A Pack could not be encoded, a staged copy could not be decoded, or a
    /// key could not be drawn, wrapped or unwrapped.
    ///
    /// The staged copies are this device's own ciphertext, written and read
    /// back within the run, so one that does not decode — or an Entry of it
    /// the new Pack's encoder refuses — is this device failing, not a verdict
    /// about a Pack of the Library.
    Format(coffret_format::Error),
    /// The run entered the commit flow and did not come through it.
    ///
    /// Including the catch-up and the read of the committed Keyring, which are
    /// the commit flow's routines (spec: CK-9, KL-1). Where the run reached its
    /// commit, the failure carries every Keyring repair the commit performed
    /// before it failed (spec: KL-15).
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
    /// The provider's digest of a new Pack it stored is not the digest of the
    /// bytes that were sent.
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
    /// The committed Keyring says nothing at all about a current Pack the
    /// repack has to rewrite: neither an envelope nor a key-lost marker, which
    /// KL-7 does not admit at a commit boundary — the catalog and the Keyring
    /// disagreeing about what is current (spec: KL-7).
    UnmappedContainer {
        /// The Container the key table does not mention.
        container_id: ContainerId,
    },
    /// A Pack the run had to rewrite could not be read and verified, so
    /// nothing was committed (spec: PK-23).
    ///
    /// Boxed because a refusal can carry the format layer's error, and an
    /// error that large would make every `Result` of the flow carry it too.
    Refused(Box<RepackRefusal>),
}

impl fmt::Display for RepackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(_) => f.write_str("the repack did not get what it asked of Storage"),
            Self::Index(_) => f.write_str("the repack could not read or write the Index"),
            Self::Format(_) => f.write_str(
                "the repack could not encode a Pack, decode its own staged copy, or draw or \
                 wrap a key",
            ),
            Self::Commit(_) => f.write_str("the repack did not come through the commit flow"),
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
            Self::Refused(refusal) => refusal.fmt(f),
        }
    }
}

impl error::Error for RepackError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            Self::Index(error) => Some(error),
            Self::Format(error) => Some(error),
            Self::Commit(error) => Some(error),
            Self::Io { cause, .. } => Some(cause),
            // The refusal's own sentence is this one's, so what it hands on is
            // what it would hand on itself.
            Self::Refused(refusal) => refusal.source(),
            Self::TransferCorrupted { .. } | Self::UnmappedContainer { .. } => None,
        }
    }
}

impl Redacted for RepackError {
    /// No local name of anything, and no Entry Path: the shape of the failure
    /// kept whole.
    fn redacted(&self) -> String {
        match self {
            Self::Storage(error) => format!("Repack::Storage: {}", error.redacted()),
            Self::Index(error) => format!("Repack::Index: {}", error.redacted()),
            Self::Format(error) => format!("Repack::Format: {}", error.redacted()),
            Self::Commit(error) => format!("Repack::Commit: {}", error.redacted()),
            Self::Io {
                operation, cause, ..
            } => format!("Repack::Io(operation={operation}, kind={:?})", cause.kind()),
            Self::TransferCorrupted { container_id, .. } => {
                format!("Repack::TransferCorrupted(container={container_id})")
            }
            Self::UnmappedContainer { container_id } => {
                format!("Repack::UnmappedContainer(container={container_id})")
            }
            Self::Refused(refusal) => format!("Repack::Refused: {}", refusal.redacted()),
        }
    }
}

impl From<Error> for RepackError {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}

impl From<IndexError> for RepackError {
    fn from(error: IndexError) -> Self {
        Self::Index(error)
    }
}

impl From<coffret_format::Error> for RepackError {
    fn from(error: coffret_format::Error) -> Self {
        Self::Format(error)
    }
}

impl From<CommitError> for RepackError {
    fn from(error: CommitError) -> Self {
        Self::Commit(error.into())
    }
}

impl From<CommitFailure> for RepackError {
    fn from(failure: CommitFailure) -> Self {
        Self::Commit(failure)
    }
}

impl From<LocalIoError> for RepackError {
    fn from(error: LocalIoError) -> Self {
        Self::Io {
            operation: error.operation,
            path: error.path,
            cause: error.cause,
        }
    }
}

impl From<RepackRefusal> for RepackError {
    fn from(refusal: RepackRefusal) -> Self {
        Self::Refused(Box::new(refusal))
    }
}

impl From<RebuildError> for RepackError {
    fn from(error: RebuildError) -> Self {
        match error {
            RebuildError::Storage(error) => Self::Storage(error),
            RebuildError::Index(error) => Self::Index(error),
            RebuildError::Local(error) => Self::from(error),
            RebuildError::Format(error) => Self::Format(error),
        }
    }
}

impl From<PackSpoolError> for RepackError {
    fn from(error: PackSpoolError) -> Self {
        match error {
            PackSpoolError::Index(error) => Self::Index(error),
            PackSpoolError::Local(error) => Self::from(error),
            PackSpoolError::Format(error) => Self::Format(error),
        }
    }
}

impl From<UploadError> for RepackError {
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
