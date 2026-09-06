use std::path::{Path, PathBuf};

use crate::in_memory_fs::InMemoryFs;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync_conformance::fixtures::spool_dir;

/// What a backend hands the sync suite for one case.
///
/// One store, one catalog, and a folder. The folder is what makes this suite
/// different from the commit one: a sync starts at a folder on the device, so a
/// case needs somewhere to put files, and it is the backend's to choose — a run
/// against a real provider may want it somewhere particular — and it is handed
/// over empty.
///
/// Where the ciphertext waits is not the backend's, and that is deliberate. The
/// spool is an [`InMemoryFs`] the fixture makes for itself, because what the
/// cases about an interrupted run need of it is not a directory but a script: a
/// spool that cannot be created, cannot be flushed, or cannot be removed is
/// what the rules around a pending row are written for (spec: OC-2, OC-6), and
/// no real filesystem refuses on request. It is one spool per case, so nothing
/// a case leaves in it reaches the next.
///
/// One catalog and not two: a sync is one device carrying its own folder into
/// the Library, and what happens when two devices commit at once is the commit
/// suite's question, asked there over the flow that answers it.
pub struct SyncUnderTest {
    // Dropped before `resources`, so that whatever a catalog or a store is kept
    // in outlives them.
    store: Box<dyn ObjectStore>,
    index: Box<dyn Index>,
    folder: PathBuf,
    spool: InMemoryFs,
    resources: Vec<Box<dyn Send + Sync>>,
}

impl SyncUnderTest {
    /// Takes an empty store, an empty catalog, and an empty folder.
    pub fn new(
        store: Box<dyn ObjectStore>,
        index: Box<dyn Index>,
        folder: impl AsRef<Path>,
    ) -> Self {
        Self {
            store,
            index,
            folder: folder.as_ref().to_path_buf(),
            spool: InMemoryFs::new(),
            resources: Vec::new(),
        }
    }

    /// Keeps something alive for as long as the case runs.
    ///
    /// A backend whose folder is a temporary directory, or whose Library sits
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

    /// The catalog of the device the case syncs from.
    pub fn index(&self) -> &dyn Index {
        self.index.as_ref()
    }

    /// The folder the device maps into the Library.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Where encoded Containers wait between being written and being committed.
    ///
    /// The fake itself and not a path: a case reads what is in it, and the cases
    /// about a failing disk script it.
    pub fn spool(&self) -> &InMemoryFs {
        &self.spool
    }

    /// The directory inside that spool the runs of a case write into.
    pub fn spool_dir(&self) -> &Path {
        spool_dir()
    }
}
