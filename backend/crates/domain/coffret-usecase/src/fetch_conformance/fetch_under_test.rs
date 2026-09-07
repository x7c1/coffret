use std::path::{Path, PathBuf};

use crate::in_memory_fs::InMemoryFs;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync_conformance::fixtures::{folder, spool_dir};

/// What a backend hands the fetch suite for one case.
///
/// One store and **two devices**, and the second device is the whole point. A
/// sync can be held to its contract by one device, because what it produces is
/// on Storage either way. A fetch cannot: what it is worth is what a device that
/// did not make the Library can get out of it, so every case here syncs from one
/// catalog and folder and fetches into another. The two catalogs share nothing —
/// no mappings, no materialization records, no checkpoint — which is what makes
/// the target device's catch-up a real restore-and-replay (spec: CK-9, RV-1).
///
/// One folder is the backend's and one is not, and the asymmetry is the point of
/// the suite. The fetching device's folder is real, because a fetch places bytes
/// through the operating system and what a case asserts is what is on that disk
/// afterwards. The source device's is in the [`InMemoryFs`] this fixture makes,
/// because all that device does is scan and upload — which goes through
/// [`MappedRoots`](crate::MappedRoots) — and the disk its Containers were
/// spooled onto on the way in is not part of the question either.
pub struct FetchUnderTest {
    // Dropped before `resources`, so that whatever a catalog or a store is kept
    // in outlives them.
    store: Box<dyn ObjectStore>,
    source: Box<dyn Index>,
    target: Box<dyn Index>,
    target_folder: PathBuf,
    fs: InMemoryFs,
    resources: Vec<Box<dyn Send + Sync>>,
}

impl FetchUnderTest {
    /// Takes an empty store, two empty catalogs, and the empty folder the second
    /// device fetches into.
    pub fn new(
        store: Box<dyn ObjectStore>,
        source: Box<dyn Index>,
        target: Box<dyn Index>,
        target_folder: impl AsRef<Path>,
    ) -> Self {
        let fs = InMemoryFs::new();
        // The folder the source device syncs is there before the case starts,
        // the way a folder a person points a device at is.
        fs.create_dir(folder());
        Self {
            store,
            source,
            target,
            target_folder: target_folder.as_ref().to_path_buf(),
            fs,
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

    /// The catalog of the device that puts files into the Library.
    pub fn source(&self) -> &dyn Index {
        self.source.as_ref()
    }

    /// The folder that device syncs, inside its own disk.
    pub fn source_folder(&self) -> &Path {
        folder()
    }

    /// The catalog of the device that fetches them back out.
    pub fn target(&self) -> &dyn Index {
        self.target.as_ref()
    }

    /// The folder that device fetches into, which is a real one.
    pub fn target_folder(&self) -> &Path {
        &self.target_folder
    }

    /// The source device's whole disk: the folder it syncs and the spool its
    /// Containers wait in.
    pub fn fs(&self) -> &InMemoryFs {
        &self.fs
    }

    /// The directory inside that same disk the source device's runs write into.
    pub fn spool_dir(&self) -> &Path {
        spool_dir()
    }
}
