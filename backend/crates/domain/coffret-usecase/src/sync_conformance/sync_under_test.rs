use std::path::{Path, PathBuf};

use crate::index::Index;
use crate::object_store::ObjectStore;

/// What a backend hands the sync suite for one case.
///
/// One store, one catalog, and two directories. The second half is what makes
/// this suite different from the commit one: a sync starts at a folder on the
/// device, so a case needs somewhere to put files and somewhere for the
/// ciphertext to wait. Both are the backend's to choose — a run against a real
/// provider may want them somewhere particular — and both are handed over empty.
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
    spool: PathBuf,
    resources: Vec<Box<dyn Send + Sync>>,
}

impl SyncUnderTest {
    /// Takes an empty store, an empty catalog, and two empty directories.
    pub fn new(
        store: Box<dyn ObjectStore>,
        index: Box<dyn Index>,
        folder: impl AsRef<Path>,
        spool: impl AsRef<Path>,
    ) -> Self {
        Self {
            store,
            index,
            folder: folder.as_ref().to_path_buf(),
            spool: spool.as_ref().to_path_buf(),
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
    pub fn spool(&self) -> &Path {
        &self.spool
    }
}
