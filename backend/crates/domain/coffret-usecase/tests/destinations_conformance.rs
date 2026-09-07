//! The destinations capability's own contract, run against the in-memory fake.
//!
//! The fake is what every fetch case in this crate places through, so what it
//! promises has to be what a device's disk promises: a case that arranges a
//! symbolic link over a fake that quietly descended through it, or that reported
//! a half-written scratch file as the placed Entry, would be arranging a state
//! no device produces. The local filesystem gateway runs the same suite against
//! a real directory.
//!
//! Nothing here is on a filesystem, so the mapped root the fixture hands over is
//! any path at all — and it is one the fixture *does* make, because a mapped
//! root that is there is where every case starts.

use std::path::Path;

use coffret_model::Mtime;
use coffret_usecase::destinations_conformance::{DestinationsUnderTest, RootArrangement};
use coffret_usecase::InMemoryFs;

/// Where the cases place, inside the in-memory filesystem they run against.
const ROOT: &str = "/mapped";

/// An empty in-memory filesystem and a mapped root inside it, for one case.
///
/// The capability and the arrangement are two handles on one fake, which is the
/// shape a real backend has: the files a case plants and the files the
/// capability writes are the same files.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<DestinationsUnderTest> {
    let fs = InMemoryFs::new();
    fs.create_dir(Path::new(ROOT));

    Some(DestinationsUnderTest::new(
        Box::new(fs.clone()),
        Box::new(Arrangement { fs }),
        Path::new(ROOT),
    ))
}

/// How a case plants folders and files in the fake, and reads them back.
///
/// Every gesture is one the fake already offers, because what it models of a
/// filesystem is exactly what the flows above it can tell apart — a planted
/// "other" standing in for the symbolic link there is no filesystem here to
/// make.
struct Arrangement {
    fs: InMemoryFs,
}

impl RootArrangement for Arrangement {
    fn write_file(&self, path: &Path, bytes: &[u8], mtime: Mtime) {
        self.fs.write_file(path, bytes);
        self.fs.set_mtime(path, mtime.as_unix_seconds());
    }

    fn plant_other(&self, path: &Path) {
        self.fs.plant_other(path);
    }

    fn content(&self, path: &Path) -> Option<Vec<u8>> {
        self.fs.content(path)
    }

    fn mtime(&self, path: &Path) -> Option<Mtime> {
        self.fs.observed(path).map(|(_, mtime)| mtime)
    }

    fn holds(&self, path: &Path) -> bool {
        self.fs.holds(path)
    }
}

coffret_usecase::destinations_conformance!(fixture().await);
