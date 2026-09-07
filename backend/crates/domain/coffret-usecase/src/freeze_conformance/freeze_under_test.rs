use std::path::Path;

use crate::in_memory_fs::InMemoryFs;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync_conformance::fixtures::{folder, spool_dir};

/// What a backend hands the freeze suite for one case.
///
/// One store and **two devices**, for the reason the fetch suite takes two: what
/// packing a folder is worth is what somebody else can get out of the Packs
/// afterwards, and only a catalog that never saw the Library can prove a Pack is
/// readable from Storage alone. The freezing device drives every case; the
/// second one fetches at the end of the round-trip case, with an empty catalog
/// of its own so its catch-up is a real restore-and-replay (spec: CK-9, RV-1).
///
/// Neither folder is the backend's, and neither is on a filesystem. Both live in
/// the one [`InMemoryFs`] this fixture makes, because everything either device
/// does to a local file goes through a capability: the freezing device walks and
/// spools through [`MappedRoots`](crate::MappedRoots) and
/// [`Spool`](crate::Spool), and the fetching device places through
/// [`Destinations`](crate::Destinations). What the cases need of either folder
/// is a script rather than a directory: a root that cannot be stated, a folder
/// that cannot be listed, a member that cannot be read (spec: EP-8, EP-12,
/// OC-2).
pub struct FreezeUnderTest {
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
/// Any path at all, and deliberately not under the freezing device's: a case
/// that mapped the two onto one folder would have the second device fetching
/// what the first one packed.
const TARGET_FOLDER: &str = "/target";

impl FreezeUnderTest {
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

    /// The catalog of the device that packs its folder into the Library.
    pub fn source(&self) -> &dyn Index {
        self.source.as_ref()
    }

    /// The folder that device freezes, inside its own disk.
    pub fn source_folder(&self) -> &Path {
        folder()
    }

    /// The catalog of the device that reads the Packs back out.
    pub fn target(&self) -> &dyn Index {
        self.target.as_ref()
    }

    /// The folder that device fetches into, inside the same disk.
    pub fn target_folder(&self) -> &Path {
        Path::new(TARGET_FOLDER)
    }

    /// The whole disk under both devices: the folder one packs, the spool it
    /// writes, and the folder the other places into.
    ///
    /// The fake itself and not a path, because a case reads what is in it and
    /// the cases about a disk that refuses script it.
    pub fn fs(&self) -> &InMemoryFs {
        &self.fs
    }

    /// The directory inside that same disk the runs of a case spool into.
    pub fn spool_dir(&self) -> &Path {
        spool_dir()
    }
}
