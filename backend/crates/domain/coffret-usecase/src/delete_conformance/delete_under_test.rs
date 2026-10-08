use std::path::Path;

use crate::in_memory_fs::InMemoryFs;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync_conformance::fixtures::{folder, spool_dir};

/// What a backend hands the deletion suite for one case.
///
/// One store and **two devices**. A deletion reads no mapped folder, but a case
/// has to put Entries into the Library before it can take any out, and the
/// flows that do that start at a folder — so the first device has one, in the
/// [`InMemoryFs`] this fixture makes, along with the spool every flow here
/// writes into. The second device maps nothing and has materialized nothing:
/// it is the device that deletes Entries it never held, which is what a
/// rebuild's bookkeeping must not mistake for files it put on disk
/// (spec: EP-10, OC-7). Its catalog starts empty, so its first catch-up is a
/// real restore-and-replay (spec: CK-9).
pub struct DeleteUnderTest {
    // Dropped before `resources`, so that whatever a catalog or a store is kept
    // in outlives them.
    store: Box<dyn ObjectStore>,
    index: Box<dyn Index>,
    other: Box<dyn Index>,
    fs: InMemoryFs,
    resources: Vec<Box<dyn Send + Sync>>,
}

impl DeleteUnderTest {
    /// Takes an empty store and two empty catalogs.
    pub fn new(store: Box<dyn ObjectStore>, index: Box<dyn Index>, other: Box<dyn Index>) -> Self {
        let fs = InMemoryFs::new();
        fs.create_dir(folder());
        Self {
            store,
            index,
            other,
            fs,
            resources: Vec::new(),
        }
    }

    /// Keeps something alive for as long as the case runs.
    pub fn holding(mut self, resource: Box<dyn Send + Sync>) -> Self {
        self.resources.push(resource);
        self
    }

    /// The Storage the Library lives in.
    pub fn store(&self) -> &dyn ObjectStore {
        self.store.as_ref()
    }

    /// The catalog of the device that fills the Library and deletes from it.
    pub fn index(&self) -> &dyn Index {
        self.index.as_ref()
    }

    /// The catalog of the device that holds none of the Library's files.
    pub fn other(&self) -> &dyn Index {
        self.other.as_ref()
    }

    /// The folder the first device carries into the Library.
    pub fn folder(&self) -> &Path {
        folder()
    }

    /// The device's disk: the folder and the spool alike.
    pub fn fs(&self) -> &InMemoryFs {
        &self.fs
    }

    /// Where every run of a case spools.
    pub fn spool_dir(&self) -> &Path {
        spool_dir()
    }
}
