use std::path::{Path, PathBuf};

use crate::index::Index;
use crate::object_store::ObjectStore;

/// What a backend hands the fetch suite for one case.
///
/// One store and **two devices**, and the second device is the whole point. A
/// sync can be held to its contract by one device, because what it produces is
/// on Storage either way. A fetch cannot: what it is worth is what a device that
/// did not make the Library can get out of it, so every case here syncs from one
/// catalog and folder and fetches into another. The two catalogs share nothing —
/// no mappings, no materialization records, no checkpoint — which is what makes
/// the target device's catch-up a real restore-and-replay rather than a
/// no-op (spec: CK-9, RV-1).
///
/// Both folders and the spool directory are the backend's to choose, because a
/// run against a real provider may want them somewhere particular, and all three
/// are handed over empty.
pub struct FetchUnderTest {
    // Dropped before `resources`, so that whatever a catalog or a store is kept
    // in outlives them.
    store: Box<dyn ObjectStore>,
    source: Box<dyn Index>,
    source_folder: PathBuf,
    target: Box<dyn Index>,
    target_folder: PathBuf,
    spool: PathBuf,
    resources: Vec<Box<dyn Send + Sync>>,
}

impl FetchUnderTest {
    /// Takes an empty store, two empty catalogs, and three empty directories.
    pub fn new(
        store: Box<dyn ObjectStore>,
        source: Box<dyn Index>,
        source_folder: impl AsRef<Path>,
        target: Box<dyn Index>,
        target_folder: impl AsRef<Path>,
        spool: impl AsRef<Path>,
    ) -> Self {
        Self {
            store,
            source,
            source_folder: source_folder.as_ref().to_path_buf(),
            target,
            target_folder: target_folder.as_ref().to_path_buf(),
            spool: spool.as_ref().to_path_buf(),
            resources: Vec::new(),
        }
    }

    /// Keeps something alive for as long as the case runs.
    ///
    /// A backend whose folders are temporary directories, or whose Library sits
    /// under a key prefix it wants cleaned up, hands the owner over here rather
    /// than leaking it.
    pub fn holding(mut self, resource: Box<dyn Send + Sync>) -> Self {
        self.resources.push(resource);
        self
    }

    /// The Storage the Library lives in.
    pub fn store(&self) -> &dyn ObjectStore {
        self.store.as_ref()
    }

    /// The catalog of the device that puts files into the Library.
    pub fn source(&self) -> &dyn Index {
        self.source.as_ref()
    }

    /// The folder that device syncs.
    pub fn source_folder(&self) -> &Path {
        &self.source_folder
    }

    /// The catalog of the device that fetches them back out.
    pub fn target(&self) -> &dyn Index {
        self.target.as_ref()
    }

    /// The folder that device fetches into.
    pub fn target_folder(&self) -> &Path {
        &self.target_folder
    }

    /// Where the source device's encoded Containers wait between being written
    /// and being committed.
    pub fn spool(&self) -> &Path {
        &self.spool
    }
}
