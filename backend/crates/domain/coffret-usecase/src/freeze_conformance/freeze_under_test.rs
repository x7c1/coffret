use std::path::{Path, PathBuf};

use crate::index::Index;
use crate::object_store::ObjectStore;

/// What a backend hands the freeze suite for one case.
///
/// One store and **two devices**, for the reason the fetch suite takes two: what
/// packing a folder is worth is what somebody else can get out of the Packs
/// afterwards, and only a catalog that never saw the Library can prove a Pack is
/// readable from Storage alone. The freezing device drives every case; the
/// second one fetches at the end of the round-trip case, with an empty catalog
/// of its own so its catch-up is a real restore-and-replay (spec: CK-9, RV-1).
///
/// Both folders and the spool directory are the backend's to choose, because a
/// run against a real provider may want them somewhere particular, and all three
/// are handed over empty.
pub struct FreezeUnderTest {
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

impl FreezeUnderTest {
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

    /// The catalog of the device that packs its folder into the Library.
    pub fn source(&self) -> &dyn Index {
        self.source.as_ref()
    }

    /// The folder that device freezes.
    pub fn source_folder(&self) -> &Path {
        &self.source_folder
    }

    /// The catalog of the device that reads the Packs back out.
    pub fn target(&self) -> &dyn Index {
        self.target.as_ref()
    }

    /// The folder that device fetches into.
    pub fn target_folder(&self) -> &Path {
        &self.target_folder
    }

    /// Where encoded Packs wait between being written and being committed.
    pub fn spool(&self) -> &Path {
        &self.spool
    }
}
