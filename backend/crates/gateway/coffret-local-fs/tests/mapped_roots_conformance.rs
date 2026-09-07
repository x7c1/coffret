//! The mapped-roots capability's own contract, run against a real directory.
//!
//! The same suite the use-case crate runs against its in-memory fake, so that
//! the fake a flow case is scanned over and the folders a device actually reads
//! are held to one contract. A case that passes there and fails here is this
//! gateway's disagreement with the capability, not the flow's with itself.
//!
//! Unix-only, and by one case: what "neither a file nor a folder" means on a
//! real filesystem is a symbolic link, and making one is
//! `std::os::unix::fs::symlink`. The fake has a planted marker instead, which is
//! why the suite itself is portable and this target is not.
//!
//! The directory is a temporary one this target owns, so an ordinary
//! `cargo test` needs no state directory and leaves nothing behind.

#![cfg(unix)]

use std::fs::FileTimes;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use coffret_local_fs::UnixFs;
use coffret_model::Mtime;
use coffret_usecase::mapped_roots_conformance::{FolderArrangement, MappedRootsUnderTest};
use tempfile::TempDir;

/// A real filesystem and a folder that already exists, for one case.
///
/// Made, unlike the spool suite's directory: a mapped root that is there is
/// where every case starts, and the cases about one that is *not* name a path
/// under it that nothing created.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<MappedRootsUnderTest> {
    let directory = TempDir::new().expect("making a temporary directory must succeed");
    let dir = directory.path().join("folder");
    std::fs::create_dir_all(&dir).expect("making the mapped folder must succeed");

    Some(
        MappedRootsUnderTest::new(Box::new(UnixFs::new()), Box::new(Arrangement), dir)
            // Dropping it removes the directory, so a case that panics leaves
            // nothing behind either.
            .holding(Box::new(directory)),
    )
}

/// How a case plants folders and files on the real filesystem.
///
/// Blocking calls rather than the runtime's, because arranging a folder is what
/// happens before a run starts: nothing here is what the capability under test
/// does.
struct Arrangement;

impl FolderArrangement for Arrangement {
    fn create_dir(&self, path: &Path) {
        std::fs::create_dir_all(path).expect("making a folder must succeed");
    }

    fn write_file(&self, path: &Path, bytes: &[u8], mtime: Mtime) {
        self.create_dir(path.parent().expect("a case's file sits in a folder"));
        std::fs::write(path, bytes).expect("writing a file must succeed");

        // Set outright rather than left as the clock found it, because what the
        // listing reports about it is one of the things being asserted.
        let file = std::fs::File::options()
            .write(true)
            .open(path)
            .expect("opening a file to stamp it must succeed");
        let seconds = Duration::from_secs(mtime.as_unix_seconds().unsigned_abs());
        file.set_times(FileTimes::new().set_modified(UNIX_EPOCH + seconds))
            .expect("setting a modification time must succeed");
    }

    /// A symbolic link, which is the shape EP-8 is actually about.
    ///
    /// Dangling deliberately: a listing that followed it would fail the stat
    /// rather than quietly report the wrong file, so the case says the same
    /// thing either way and this one costs no second file.
    fn plant_other(&self, path: &Path) {
        self.create_dir(path.parent().expect("a case's link sits in a folder"));
        std::os::unix::fs::symlink("nowhere-in-particular", path)
            .expect("making a symbolic link must succeed");
    }

    fn remove_dir_all(&self, path: &Path) {
        std::fs::remove_dir_all(path).expect("removing a folder must succeed");
    }
}

coffret_usecase::mapped_roots_conformance!(fixture().await);
