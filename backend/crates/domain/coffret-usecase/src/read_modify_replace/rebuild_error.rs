use crate::error::Error;
use crate::index_error::IndexError;
use crate::local_io_error::LocalIoError;

/// What stops a rebuild without being a verdict about the Container it reads.
///
/// The other answer a rebuild gives is [`Unverified`](super::Unverified), which
/// costs the one Container and nothing else. These cost the run: a Storage that
/// keeps failing after the retry policy gave up, a catalog that cannot be
/// written, a spool that cannot be, or a key that cannot be drawn or wrapped.
/// None of them says anything about whether the old Container is what the
/// Library says it is, so none of them may be reported as if it did — and each
/// flow that rebuilds carries them under its own names.
#[derive(Debug)]
pub(crate) enum RebuildError {
    /// Storage failed, or answered something the run cannot go on from.
    Storage(Error),
    /// The Index could not be read or written.
    Index(IndexError),
    /// The spool could not be created, written, flushed, or removed.
    Local(LocalIoError),
    /// A key could not be drawn or wrapped, or the replacement could not be
    /// encoded.
    Format(coffret_format::Error),
}

impl From<Error> for RebuildError {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}

impl From<IndexError> for RebuildError {
    fn from(error: IndexError) -> Self {
        Self::Index(error)
    }
}

impl From<LocalIoError> for RebuildError {
    fn from(error: LocalIoError) -> Self {
        Self::Local(error)
    }
}

impl From<coffret_format::Error> for RebuildError {
    fn from(error: coffret_format::Error) -> Self {
        Self::Format(error)
    }
}
