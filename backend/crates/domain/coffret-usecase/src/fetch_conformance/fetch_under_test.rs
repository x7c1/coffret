use std::path::Path;

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
/// Neither folder is the backend's, and neither is on a filesystem. Both live in
/// the one [`InMemoryFs`] this fixture makes, because everything either device
/// does to a local file goes through a capability: the source device scans and
/// spools through [`MappedRoots`](crate::MappedRoots) and
/// [`Spool`](crate::Spool), and the target device places through
/// [`Destinations`](crate::Destinations). One fake for both, because what the
/// cases about a disk that refuses need is a disk that can be told to — and
/// which device's folder a case is reading is the path it names rather than
/// which object it asks.
pub struct FetchUnderTest {
    // Dropped before `resources`, so that whatever a catalog or a store is kept
    // in outlives them.
    store: Box<dyn ObjectStore>,
    source: Box<dyn Index>,
    target: Box<dyn Index>,
    fs: InMemoryFs,
    resources: Vec<Box<dyn Send + Sync>>,
}

/// Where the fetching device's folder stands inside the fake.
///
/// Any path at all, and deliberately not under the source device's: a case that
/// mapped the two onto one folder would have the target device fetching what the
/// source device is still scanning.
const TARGET_FOLDER: &str = "/target";

impl FetchUnderTest {
    /// Takes an empty store and two empty catalogs.
    ///
    /// No folder: both devices' folders are inside the fake this makes, so
    /// there is nothing for a backend to hand over and nothing for it to clean
    /// up.
    pub fn new(
        store: Box<dyn ObjectStore>,
        source: Box<dyn Index>,
        target: Box<dyn Index>,
    ) -> Self {
        let fs = InMemoryFs::new();
        // Both folders are there before the case starts, the way a folder a
        // person points a device at is.
        fs.create_dir(folder());
        fs.create_dir(Path::new(TARGET_FOLDER));
        Self {
            store,
            source,
            target,
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

    /// The folder that device fetches into, inside the same disk.
    pub fn target_folder(&self) -> &Path {
        Path::new(TARGET_FOLDER)
    }

    /// The whole disk under both devices: the folder one syncs, the spool its
    /// Containers wait in, and the folder the other places into.
    ///
    /// The fake itself and not a path, because a case reads what is in it and
    /// the cases about a disk that refuses script it.
    pub fn fs(&self) -> &InMemoryFs {
        &self.fs
    }

    /// The directory inside that same disk the source device's runs write into.
    pub fn spool_dir(&self) -> &Path {
        spool_dir()
    }
}
