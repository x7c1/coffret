//! What putting a Library on this device reaches past this crate for.
//!
//! Two things, and they are the two a case cannot otherwise stand in for. The
//! catalog is a file the concrete [`SqliteIndex`] creates, so a catalog that will
//! not open is a disk refusing on cue — which no case can arrange of a real one.
//! And every call to Drive goes out through a transport, which in the shipping
//! build is a real HTTPS client aimed at Google; a case that went through it
//! would need a person's consent and a browser to give it in.
//!
//! So both flows take them from here rather than building them where they are
//! used, and what ships builds them the one way it always has:
//! [`Reach::this_device`] is the only constructor outside the cases.

use std::path::Path;
use std::sync::Arc;

use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::IndexResult;
use google_drive_store::HttpTransport;

use crate::drive;
use crate::error::Result;

/// Opens (creating it where there is none) the catalog file at a path.
///
/// Opened and let go: a Library directory is given its catalog when it is
/// staged, and what reads it later opens it again.
pub(crate) type IndexOpener = fn(&Path) -> IndexResult<()>;

/// What a flow that puts a Library on this device builds its catalog and its
/// Drive calls from.
pub(crate) struct Reach {
    open_index: IndexOpener,
    drive: DriveTransport,
}

/// Where a Drive call goes out through.
enum DriveTransport {
    /// Built when a Drive flow first needs one, and only then: an S3 Library
    /// never builds an HTTPS client it would not use.
    Built,
    /// Handed over whole, by a case.
    #[cfg(test)]
    Given(Arc<dyn HttpTransport>),
}

impl Reach {
    /// The catalog this build ships, and a real transport to Drive.
    pub(crate) fn this_device() -> Self {
        Self {
            open_index: |path| SqliteIndex::open(path).map(drop),
            drive: DriveTransport::Built,
        }
    }

    /// The same, with the catalog opened by `open_index` instead.
    ///
    /// The seam a case over a catalog that will not open goes through: the flow
    /// runs exactly as it ships up to the one call that is refused.
    #[cfg(test)]
    pub(crate) fn opening_index_with(self, open_index: IndexOpener) -> Self {
        Self { open_index, ..self }
    }

    /// The same, with every call to Drive going out through `transport`.
    #[cfg(test)]
    pub(crate) fn reaching_drive_through(self, transport: Arc<dyn HttpTransport>) -> Self {
        Self {
            drive: DriveTransport::Given(transport),
            ..self
        }
    }

    /// Opens the catalog file a Library directory is given.
    pub(crate) fn open_index(&self, path: &Path) -> IndexResult<()> {
        (self.open_index)(path)
    }

    /// The transport a Drive flow's calls go out through.
    pub(crate) fn drive_transport(&self) -> Result<Arc<dyn HttpTransport>> {
        match &self.drive {
            DriveTransport::Built => drive::transport(),
            #[cfg(test)]
            DriveTransport::Given(transport) => Ok(Arc::clone(transport)),
        }
    }
}
