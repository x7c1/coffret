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
//!
//! Creating a Library also writes its first head to Storage, and joining one
//! replays its Journal from there, and a case over a stood-in Drive transport
//! would otherwise need that transport to answer the whole Storage port. So the
//! store those control objects are written to and read from is the third thing
//! a case may stand in, and what ships always reaches the Storage the settings
//! name.

use std::path::Path;
use std::sync::Arc;

use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::{IndexResult, ObjectStore};
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
    /// The Storage a Library's control objects are written to and read from
    /// while it is put on this device, where a case stands one in for the
    /// provider's own.
    #[cfg(test)]
    storage: Option<Arc<dyn ObjectStore>>,
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
            #[cfg(test)]
            storage: None,
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
    ///
    /// A transport stood in answers what setting a Library up asks of Drive and
    /// not the whole Storage port, so a new Library's first head goes to a
    /// store in memory that mints identifiers as Drive does, unless the case
    /// has stood in a store of its own.
    #[cfg(test)]
    pub(crate) fn reaching_drive_through(self, transport: Arc<dyn HttpTransport>) -> Self {
        let storage = self.storage.or_else(|| {
            Some(Arc::new(coffret_usecase::InMemoryMintingStore::new(16)) as Arc<dyn ObjectStore>)
        });
        Self {
            drive: DriveTransport::Given(transport),
            storage,
            ..self
        }
    }

    /// The same, with a new Library's first head written to `store`, and a
    /// joined one's Journal read from it, instead of the Storage its settings
    /// name.
    #[cfg(test)]
    pub(crate) fn storing_into(self, store: Arc<dyn ObjectStore>) -> Self {
        Self {
            storage: Some(store),
            ..self
        }
    }

    /// The Storage a new Library's first head goes to, and a joined one's
    /// Journal is read from: `built`, the store over the place its settings
    /// name, unless a case stood another one in.
    pub(crate) fn library_store(&self, built: Arc<dyn ObjectStore>) -> Arc<dyn ObjectStore> {
        #[cfg(test)]
        if let Some(store) = &self.storage {
            return Arc::clone(store);
        }
        built
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
