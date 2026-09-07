use std::path::{Path, PathBuf};

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
/// One folder and not two, and it is the fetching device's. The freezing
/// device's is not the backend's any more: the walk goes through
/// [`MappedRoots`](crate::MappedRoots), so the folder a case packs lives in the
/// [`InMemoryFs`] this fixture makes — which is also where the ciphertext waits,
/// because a device has one disk. What the cases need of that folder is a
/// script rather than a directory: a root that cannot be stated, a folder that
/// cannot be listed, a member that cannot be read (spec: EP-8, EP-12, OC-2).
///
/// The fetching device's folder is still real, and has to be: a fetch places
/// bytes through the operating system, and what the round-trip case asserts is
/// what is on that disk afterwards.
pub struct FreezeUnderTest {
    // Dropped before `resources`, so that whatever a catalog or a store is kept
    // in outlives them.
    store: Box<dyn ObjectStore>,
    source: Box<dyn Index>,
    target: Box<dyn Index>,
    target_folder: PathBuf,
    fs: InMemoryFs,
    resources: Vec<Box<dyn Send + Sync>>,
}

impl FreezeUnderTest {
    /// Takes an empty store, two empty catalogs, and the empty folder the second
    /// device fetches into.
    pub fn new(
        store: Box<dyn ObjectStore>,
        source: Box<dyn Index>,
        target: Box<dyn Index>,
        target_folder: impl AsRef<Path>,
    ) -> Self {
        let fs = InMemoryFs::new();
        // The folder the freezing device packs is there before the case starts,
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

    /// The folder that device fetches into, which is a real one.
    pub fn target_folder(&self) -> &Path {
        &self.target_folder
    }

    /// The freezing device's whole disk: the folder it packs and the spool it
    /// writes.
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
