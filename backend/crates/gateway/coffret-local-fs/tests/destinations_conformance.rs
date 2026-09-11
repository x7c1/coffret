//! The destinations capability's own contract, run against a real directory.
//!
//! The same suite the use-case crate runs against its in-memory fake, so that
//! the fake a fetch case is driven over and the folders a device actually places
//! into are held to one contract. A case that passes there and fails here is
//! this gateway's disagreement with the capability, not the flow's with itself.
//!
//! Unix-only, and by more than one case: what "neither a file nor a folder"
//! means on a real filesystem is a symbolic link, and making one is
//! `std::os::unix::fs::symlink` — and the descent behind the whole capability is
//! `openat` with `O_NOFOLLOW` and `O_DIRECTORY`. The fake has a planted "other"
//! and a map instead, which is why the suite itself is portable and this target
//! is not.
//!
//! The directory is a temporary one this target owns, so an ordinary
//! `cargo test` needs no state directory and leaves nothing behind.

#![cfg(unix)]

use std::fs::FileTimes;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use coffret_local_fs::UnixFs;
use coffret_model::Mtime;
use coffret_usecase::destinations_conformance::{DestinationsUnderTest, RootArrangement};
use tempfile::TempDir;

/// A real filesystem and a mapped root that already exists, for one case.
///
/// Made, because a mapped root is the folder a person pointed this device at and
/// every case starts from one that is there. The folders *below* it are the
/// capability's own to make (spec: EP-2).
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<DestinationsUnderTest> {
    let directory = TempDir::new().expect("making a temporary directory must succeed");
    let root = directory.path().join("mapped");
    std::fs::create_dir_all(&root).expect("making the mapped root must succeed");

    Some(
        DestinationsUnderTest::new(Box::new(UnixFs::new()), Box::new(Arrangement), root)
            // Dropping it removes the directory, so a case that panics leaves
            // nothing behind either.
            .holding(Box::new(directory)),
    )
}

/// How a case plants folders and files on the real filesystem, and reads them
/// back.
///
/// Blocking calls rather than the runtime's, because arranging a folder is what
/// happens before a run starts and reading it back is what happens after:
/// nothing here is what the capability under test does.
struct Arrangement;

impl Arrangement {
    /// Makes a folder, and the folders above it, for the two arranging calls
    /// that put something inside one.
    fn create_dir(&self, path: &Path) {
        std::fs::create_dir_all(path).expect("making a folder must succeed");
    }
}

impl RootArrangement for Arrangement {
    fn write_file(&self, path: &Path, bytes: &[u8], mtime: Mtime) {
        self.create_dir(path.parent().expect("a case's file sits in a folder"));
        std::fs::write(path, bytes).expect("writing a file must succeed");

        // Set outright rather than left as the clock found it, because what a
        // look reports about it is one of the things being asserted.
        let file = std::fs::File::options()
            .write(true)
            .open(path)
            .expect("opening a file to stamp it must succeed");
        let seconds = Duration::from_secs(mtime.as_unix_seconds().unsigned_abs());
        file.set_times(FileTimes::new().set_modified(UNIX_EPOCH + seconds))
            .expect("setting a modification time must succeed");
    }

    /// A symbolic link, which is the shape EP-4 is actually about.
    ///
    /// Dangling deliberately: a descent that followed it would fail on what it
    /// points at rather than refusing the name, so the case says the same thing
    /// either way and this one costs no second folder.
    fn plant_other(&self, path: &Path) {
        self.create_dir(path.parent().expect("a case's link sits in a folder"));
        std::os::unix::fs::symlink("nowhere-in-particular", path)
            .expect("making a symbolic link must succeed");
    }

    fn content(&self, path: &Path) -> Option<Vec<u8>> {
        // `symlink_metadata` first, so a name that is a link answers `None`
        // rather than whatever it points at (spec: EP-8).
        if !std::fs::symlink_metadata(path).is_ok_and(|stated| stated.is_file()) {
            return None;
        }
        Some(std::fs::read(path).expect("reading a file that is there must succeed"))
    }

    fn mtime(&self, path: &Path) -> Option<Mtime> {
        let stated = std::fs::symlink_metadata(path).ok()?;
        let modified = stated
            .modified()
            .expect("the filesystem keeps modification times")
            .duration_since(UNIX_EPOCH)
            .expect("a case's files are stamped after the epoch");
        Some(Mtime::from_unix_seconds(
            i64::try_from(modified.as_secs()).expect("a time within this century"),
        ))
    }

    fn holds(&self, path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok()
    }
}

coffret_usecase::destinations_conformance!(fixture().await);
