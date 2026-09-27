use coffret_model::ContainerId;

use crate::error::Error;
use crate::index_error::IndexError;

/// What putting a batch's Containers on Storage can fail with.
///
/// Two of these are a port's verdict travelling unchanged, and one is this
/// step's own: a provider that reports a digest disagreeing with what was sent.
/// Each flow's public error type carries all three under its own names.
#[derive(Debug)]
pub(crate) enum UploadError {
    /// Storage failed, or answered something the run cannot go on from.
    Storage(Error),
    /// The Index could not be written.
    Index(IndexError),
    /// The provider's digest of a Container it stored is not the digest of the
    /// bytes that were sent.
    TransferCorrupted {
        /// The Container whose object did not arrive whole.
        container_id: ContainerId,
        /// The digest taken while the spool was written.
        expected: String,
        /// The digest the provider reports for what it stored.
        actual: String,
    },
}

impl From<Error> for UploadError {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}

impl From<IndexError> for UploadError {
    fn from(error: IndexError) -> Self {
        Self::Index(error)
    }
}
