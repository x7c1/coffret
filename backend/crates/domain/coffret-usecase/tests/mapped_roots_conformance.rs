//! The mapped-roots capability's own contract, run against the in-memory fake.
//!
//! The fake is what every flow case in this crate scans through, so what it
//! promises has to be what a device's disk promises: a case that arranges an
//! unavailable root over a fake that answered a missing folder with an empty
//! listing, or that reported a symbolic link as the file it points at, would be
//! arranging a state no device produces. The local filesystem gateway runs the
//! same suite against a real directory.
//!
//! Nothing here is on a filesystem, so the directory the fixture hands over is
//! any path at all — and it is one the fixture *does* make, because a mapped
//! root that is there is where every case starts.

use std::path::Path;

use coffret_model::Mtime;
use coffret_usecase::mapped_roots_conformance::{FolderArrangement, MappedRootsUnderTest};
use coffret_usecase::InMemoryFs;

/// Where the cases work, inside the in-memory filesystem they run against.
const DIR: &str = "/folder";

/// An empty in-memory filesystem and a folder inside it, for one case.
///
/// The capability and the arrangement are two handles on one fake, which is the
/// shape a real backend has: the files a case plants and the files the
/// capability reads are the same files.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<MappedRootsUnderTest> {
    let fs = InMemoryFs::new();
    fs.create_dir(Path::new(DIR));

    Some(MappedRootsUnderTest::new(
        Box::new(fs.clone()),
        Box::new(Arrangement { fs }),
        Path::new(DIR),
    ))
}

/// How a case plants folders and files in the fake.
///
/// Every gesture is one the fake already offers, because what it models of a
/// filesystem is exactly what the flows above it can tell apart — a planted
/// "other" standing in for the symbolic link there is no filesystem here to
/// make.
struct Arrangement {
    fs: InMemoryFs,
}

impl FolderArrangement for Arrangement {
    fn create_dir(&self, path: &Path) {
        self.fs.create_dir(path);
    }

    fn write_file(&self, path: &Path, bytes: &[u8], mtime: Mtime) {
        self.fs.write_file(path, bytes);
        self.fs.set_mtime(path, mtime.as_unix_seconds());
    }

    fn plant_other(&self, path: &Path) {
        self.fs.plant_other(path);
    }

    fn remove_dir_all(&self, path: &Path) {
        self.fs.remove_dir_all(path);
    }
}

coffret_usecase::mapped_roots_conformance!(fixture().await);
