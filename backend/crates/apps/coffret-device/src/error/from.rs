//! What a lower layer's failure becomes in this crate's vocabulary, so that `?`
//! carries one into the other.

use coffret_usecase::commit::CommitError;
use coffret_usecase::fetch::FetchError;
use coffret_usecase::freeze::FreezeError;
use coffret_usecase::sync::SyncError;
use coffret_usecase::LocalIoError;

use super::Error;

impl From<LocalIoError> for Error {
    /// The same refusal in this crate's vocabulary, which is the use case's
    /// vocabulary for this device's disk: nothing is decided on the way, so
    /// `?` carries one into the other.
    fn from(refused: LocalIoError) -> Self {
        Self::Local(refused)
    }
}

impl From<coffret_usecase::IndexError> for Error {
    fn from(cause: coffret_usecase::IndexError) -> Self {
        Self::Index { cause }
    }
}

impl From<google_drive_store::Error> for Error {
    fn from(cause: google_drive_store::Error) -> Self {
        Self::Drive {
            cause: Box::new(cause),
        }
    }
}

impl From<SyncError> for Error {
    fn from(cause: SyncError) -> Self {
        Self::Sync {
            cause: Box::new(cause),
        }
    }
}

impl From<FreezeError> for Error {
    fn from(cause: FreezeError) -> Self {
        Self::Freeze {
            cause: Box::new(cause),
        }
    }
}

impl From<FetchError> for Error {
    /// A fetch is what raises this vocabulary nearly everywhere, so the `?` in a
    /// flow means [`Fetch`](Error::Fetch). The callers that raise it without
    /// fetching anything say so outright rather than leaning on this: the EP-9
    /// translation asked on its own is
    /// [`LocalPathNotSettled`](Error::LocalPathNotSettled), a file turned
    /// away on its way into a mapped folder is
    /// [`FileNotTakenIn`](Error::FileNotTakenIn), a read of what somebody has
    /// put in a mapped folder is
    /// [`LocalFilesNotRead`](Error::LocalFilesNotRead), and the opening of the
    /// file this device placed for an Entry is
    /// [`LocalFileNotOpened`](Error::LocalFileNotOpened).
    ///
    /// Those four are every caller in this crate that speaks this vocabulary
    /// without fetching, so what is left for `?` to carry is the flows that do
    /// fetch — where the outer sentence is the true one. A `?` on this
    /// vocabulary anywhere else is a caller that has not yet said which gesture
    /// it is refusing, rather than the shape to copy.
    ///
    /// A catalog that could not be used is taken out on the way, as it is at
    /// every door onto this vocabulary.
    fn from(cause: FetchError) -> Self {
        Self::index_or(cause, |cause| Self::Fetch {
            cause: Box::new(cause),
        })
    }
}

impl From<CommitError> for Error {
    /// The one flow that reports the commit's vocabulary on its own is the
    /// catalog catch-up: every other caller of it is a sync or a fetch, and both
    /// wrap it in their own refusal before it reaches here.
    fn from(cause: CommitError) -> Self {
        Self::CatchUp {
            cause: Box::new(cause),
        }
    }
}
