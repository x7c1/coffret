use std::path::Path;

use crate::in_memory_fs::InMemoryFs;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync_conformance::fixtures::{folder, spool_dir};

/// What a backend hands the sync suite for one case.
///
/// One store and one catalog, and that is all: the folder is not the backend's.
/// The walk goes through [`MappedRoots`](crate::MappedRoots), and what the
/// cases need of a mapped folder is not a directory but a script. A root that
/// cannot be stated, a folder that cannot be listed, a source that cannot be
/// read, a root that is empty on a filesystem the mapping does not record: those
/// are what the rules around a scan are written for (spec: EP-8, EP-12), and no
/// real filesystem arranges them on request.
///
/// So the fixture makes one [`InMemoryFs`] and it is the whole device's disk —
/// the mapped folder and the spool directory are two places in it, the way they
/// are two places on a device. It is one per case, so nothing a case leaves in
/// it reaches the next.
///
/// One catalog and not two: a sync is one device carrying its own folder into
/// the Library, and what happens when two devices commit at once is the commit
/// suite's question, asked there over the flow that answers it.
pub struct SyncUnderTest {
    // Dropped before `resources`, so that whatever a catalog or a store is kept
    // in outlives them.
    store: Box<dyn ObjectStore>,
    index: Box<dyn Index>,
    fs: InMemoryFs,
    resources: Vec<Box<dyn Send + Sync>>,
}

impl SyncUnderTest {
    /// Takes an empty store and an empty catalog.
    pub fn new(store: Box<dyn ObjectStore>, index: Box<dyn Index>) -> Self {
        let fs = InMemoryFs::new();
        // The mapped folder is there before the case starts, the way a folder a
        // person points a device at is: a case about a root that is *not* there
        // names one under it that nothing created.
        fs.create_dir(folder());
        Self {
            store,
            index,
            fs,
            resources: Vec::new(),
        }
    }

    /// Keeps something alive for as long as the case runs.
    ///
    /// A backend whose Library sits under a key prefix it wants cleaned up hands
    /// the owner over here rather than leaking it.
    pub fn holding(mut self, resource: Box<dyn Send + Sync>) -> Self {
        self.resources.push(resource);
        self
    }

    /// The Storage the Library lives in.
    pub fn store(&self) -> &dyn ObjectStore {
        self.store.as_ref()
    }

    /// The catalog of the device the case syncs from.
    pub fn index(&self) -> &dyn Index {
        self.index.as_ref()
    }

    /// The device's whole disk: the mapped folders it reads and the spool it
    /// writes.
    ///
    /// The fake itself and not a path, because a case reads what is in it and
    /// the cases about a disk that refuses script it.
    pub fn fs(&self) -> &InMemoryFs {
        &self.fs
    }

    /// The folder the device maps into the Library.
    pub fn folder(&self) -> &Path {
        folder()
    }

    /// The directory inside that same disk the runs of a case spool into.
    pub fn spool_dir(&self) -> &Path {
        spool_dir()
    }
}
