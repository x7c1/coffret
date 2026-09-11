//! The mapped-roots capability's own contract, run against a real directory.
//!
//! The same suite the use-case crate runs against its in-memory fake, so that
//! the fake a flow case is scanned over and the folders a device actually reads
//! are held to one contract. A case that passes there and fails here is this
//! gateway's disagreement with the capability, not the flow's with itself.
//!
//! Unix-only, and by one case: what "neither a file nor a folder" means on a
//! real filesystem is a symbolic link, and making one is
//! `std::os::unix::fs::symlink`. The fake has a planted "other" instead,
//! which is why the suite itself is portable and this target is not.
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
use coffret_usecase::{MappedRelativeLocation, MappedRoots, SourceReader};
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

    fn replace_file(&self, path: &Path, bytes: &[u8], mtime: Mtime) {
        let replacement = path.with_extension("replacement");
        self.write_file(&replacement, bytes, mtime);
        std::fs::rename(replacement, path).expect("replacing a file must succeed");
    }

    /// A symbolic link, which is the shape EP-8 is actually about.
    ///
    /// Dangling deliberately: a listing that followed it would fail the stat
    /// rather than quietly report the wrong file, so the case says the same
    /// thing either way and this one costs no second file.
    fn plant_other(&self, path: &Path) {
        self.create_dir(path.parent().expect("a case's link sits in a folder"));
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => {}
            Err(cause) => panic!("removing the name being replaced must succeed: {cause}"),
        }
        std::os::unix::fs::symlink("nowhere-in-particular", path)
            .expect("making a symbolic link must succeed");
    }

    fn remove_dir_all(&self, path: &Path) {
        std::fs::remove_dir_all(path).expect("removing a folder must succeed");
    }
}

coffret_usecase::mapped_roots_conformance!(fixture().await);

fn mapped(path: &str) -> MappedRelativeLocation {
    MappedRelativeLocation::from_entry_path(
        &coffret_model::EntryPath::parse(path).expect("a test location is an Entry Path"),
    )
}

async fn read_all(mut reader: Box<dyn SourceReader>) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4];
    loop {
        let filled = reader.read(&mut buffer).await.expect("the open file reads");
        if filled == 0 {
            return bytes;
        }
        bytes.extend_from_slice(&buffer[..filled]);
    }
}

#[tokio::test]
async fn a_reader_refuses_parent_and_final_symbolic_links() {
    let directory = TempDir::new().expect("a temporary directory");
    let root = directory.path().join("mapped");
    let outside = directory.path().join("outside");
    std::fs::create_dir_all(&root).expect("the mapped root");
    std::fs::create_dir_all(&outside).expect("the outside folder");
    std::fs::write(outside.join("secret"), b"outside bytes").expect("the outside file");
    std::os::unix::fs::symlink(&outside, root.join("parent")).expect("the parent link");
    std::os::unix::fs::symlink(outside.join("secret"), root.join("final")).expect("the final link");

    let fs = UnixFs::new();
    for relative in ["parent/secret", "final"] {
        let refused = fs
            .open_source(&root, &mapped(relative))
            .await
            .err()
            .expect("a descendant link is never followed");
        assert!(matches!(
            refused.operation,
            coffret_usecase::LocalOperation::Reading
        ));
    }
}

#[tokio::test]
async fn opening_a_missing_source_does_not_create_its_root() {
    let directory = TempDir::new().expect("a temporary directory");
    let root = directory.path().join("missing-root");

    UnixFs::new()
        .open_source(&root, &mapped("page.jpg"))
        .await
        .err()
        .expect("the missing file is refused");

    assert!(!root.exists(), "a read never creates the configured root");
}

#[tokio::test]
async fn a_nonregular_source_is_refused_without_waiting_for_a_writer() {
    let directory = TempDir::new().expect("a temporary directory");
    let root = directory.path();
    let made = std::process::Command::new("mkfifo")
        .arg(root.join("pipe"))
        .status()
        .expect("the platform provides mkfifo");
    assert!(made.success(), "making a FIFO succeeds");

    let answer = tokio::time::timeout(
        Duration::from_secs(1),
        UnixFs::new().open_source(root, &mapped("pipe")),
    )
    .await
    .expect("opening a FIFO does not block");
    assert!(answer.is_err(), "a FIFO is not a source file");
}

#[tokio::test]
async fn an_open_reader_retains_its_file_and_handle_derived_length() {
    let directory = TempDir::new().expect("a temporary directory");
    let root = directory.path();
    let path = root.join("page.jpg");
    let original = b"first bytes";
    std::fs::write(&path, original).expect("the original file");

    let reader = UnixFs::new()
        .open_source(root, &mapped("page.jpg"))
        .await
        .expect("the original opens");
    assert_eq!(reader.len(), original.len() as u64);
    std::fs::rename(&path, root.join("old-page.jpg")).expect("the name is released");
    std::fs::write(&path, b"a replacement with a different length").expect("the replacement");

    assert_eq!(read_all(reader).await, original);
}

#[tokio::test]
async fn a_parent_replaced_after_listing_cannot_redirect_a_source_or_listing() {
    let directory = TempDir::new().expect("a temporary directory");
    let root = directory.path().join("mapped");
    let outside = directory.path().join("outside");
    std::fs::create_dir_all(root.join("album")).expect("the mapped folder");
    std::fs::create_dir_all(&outside).expect("the outside folder");
    std::fs::write(root.join("album/page.jpg"), b"inside").expect("the mapped file");
    std::fs::write(outside.join("page.jpg"), b"outside").expect("the outside file");
    let fs = UnixFs::new();
    fs.list_folder(&root, Some(&mapped("album")))
        .await
        .expect("the folder lists")
        .expect("the folder exists");

    std::fs::rename(root.join("album"), root.join("old-album")).expect("the parent moves");
    std::os::unix::fs::symlink(&outside, root.join("album")).expect("the parent link");

    assert!(fs
        .open_source(&root, &mapped("album/page.jpg"))
        .await
        .is_err());
    assert!(fs.list_folder(&root, Some(&mapped("album"))).await.is_err());
}

#[tokio::test]
async fn the_configured_root_itself_may_be_a_symbolic_link() {
    let directory = TempDir::new().expect("a temporary directory");
    let actual = directory.path().join("actual");
    let configured = directory.path().join("configured");
    std::fs::create_dir_all(&actual).expect("the actual root");
    std::fs::write(actual.join("page.jpg"), b"through the configured root").expect("the file");
    std::os::unix::fs::symlink(&actual, &configured).expect("the configured root link");

    let reader = UnixFs::new()
        .open_source(&configured, &mapped("page.jpg"))
        .await
        .expect("the configured root is deliberately followed");
    assert_eq!(read_all(reader).await, b"through the configured root");
}

#[tokio::test]
async fn a_listing_preserves_birth_time_when_the_platform_reports_one() {
    let directory = TempDir::new().expect("a temporary directory");
    let file = directory.path().join("page.jpg");
    std::fs::write(&file, b"page").expect("the file");
    let expected = std::fs::metadata(&file)
        .expect("the file is stated")
        .created()
        .ok()
        .map(|moment| match moment.duration_since(UNIX_EPOCH) {
            Ok(since) => i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
            Err(before) => i64::try_from(before.duration().as_secs())
                .map(|seconds| -seconds)
                .unwrap_or(i64::MIN),
        });

    let entries = UnixFs::new()
        .list_folder(directory.path(), None)
        .await
        .expect("the root lists")
        .expect("the root exists");
    let entry = entries
        .iter()
        .find(|entry| entry.name == "page.jpg")
        .expect("the file is listed");
    let coffret_usecase::FolderEntryKind::File { btime, .. } = entry.kind else {
        panic!("the regular file is listed as one");
    };
    assert_eq!(btime.map(|time| time.as_unix_seconds()), expected);
}

#[tokio::test]
async fn a_sync_reads_a_decomposed_filesystem_spelling_after_normalizing_its_entry_path() {
    use coffret_model::{MasterKey, MasterKeyEpoch};
    use coffret_usecase::device_state::{BatchId, DeviceTime, Mapping};
    use coffret_usecase::sync::{sync_folders, SyncRequest};
    use coffret_usecase::{InMemoryIndex, InMemoryStore, Index, LibraryKeys};

    let directory = TempDir::new().expect("a temporary directory");
    let root = directory.path().join("mapped");
    let spool = directory.path().join("spool");
    std::fs::create_dir_all(&root).expect("the mapped root");
    std::fs::write(root.join("cafe\u{301}.jpg"), b"a decomposed local name")
        .expect("the source file");
    let index = InMemoryIndex::new();
    index
        .set_mapping(Mapping::new(None, root))
        .await
        .expect("the mapping is recorded");
    let store = InMemoryStore::new(64);
    let local = UnixFs::new();
    let keys = LibraryKeys::derive(
        &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
        MasterKeyEpoch::FIRST,
    );

    let outcome = sync_folders(SyncRequest::new(
        &store,
        &index,
        &keys,
        &local,
        &local,
        spool,
        BatchId::new("decomposed-source"),
        DeviceTime::from_unix_seconds(1_700_000_000),
    ))
    .await
    .expect("the normalized Entry still reopens the filesystem's spelling");

    assert_eq!(outcome.added.len(), 1);
    assert!(index
        .entry_at(
            &coffret_model::EntryPath::parse("caf\u{e9}.jpg").expect("the normalized Entry Path"),
        )
        .await
        .expect("the catalog answers")
        .is_some(),);
}
