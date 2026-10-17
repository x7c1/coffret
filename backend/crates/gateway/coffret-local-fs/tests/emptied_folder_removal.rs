//! What removing a folder a sync's discards emptied does against a
//! real directory (spec: EP-13, EP-15).
//!
//! Unlike the discard itself, the removal never reaches the desktop's trash, so
//! the cases here can let it through: a folder with nothing in it goes, and a
//! folder holding anything at all — a dotfile among them — stays. The rest are
//! the refusals the discard makes too: a root that does not vouch for itself, and
//! a symbolic link, which is never followed and never removed in place of a
//! folder.

#![cfg(unix)]

use std::os::unix::fs::symlink;
use std::path::Path;

use coffret_local_fs::UnixFs;
use coffret_model::EntryPath;
use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::root_marker::{self, MANAGEMENT_AREA, MARKER_FILE};
use coffret_usecase::{DescentError, LocalTrash, MappedRelativeLocation, RootRefused};

/// Writes the marker a registration would have written into `root`, and hands
/// back the identity its mapping records (spec: EP-13).
fn register_root(root: &Path) -> RootMarkerId {
    let id = RootMarkerId::from_bytes([0x2a; RootMarkerId::BYTE_LEN]);
    let area = root.join(MANAGEMENT_AREA);
    std::fs::create_dir_all(&area).expect("making the management area must succeed");
    std::fs::write(area.join(MARKER_FILE), root_marker::spell(&id))
        .expect("writing the marker must succeed");
    id
}

fn below(path: &str) -> MappedRelativeLocation {
    MappedRelativeLocation::from_entry_path(
        &EntryPath::parse(path).expect("a case names a valid Entry Path"),
    )
}

#[tokio::test]
async fn a_folder_with_nothing_in_it_is_removed_and_its_parent_kept() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    std::fs::create_dir_all(root.path().join("album/sub")).expect("the folders must be creatable");

    let removed = UnixFs::new()
        .remove_emptied_folder(root.path(), Some(&id), &below("album/sub"))
        .await
        .expect("an empty folder below a vouched root must be removable");

    assert!(removed);
    assert!(!root.path().join("album/sub").exists());
    assert!(
        root.path().join("album").is_dir(),
        "only the named folder goes"
    );
}

#[tokio::test]
async fn a_folder_holding_a_dotfile_stays() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    std::fs::create_dir(root.path().join("album")).expect("the folder must be creatable");
    std::fs::write(root.path().join("album/.DS_Store"), b"the desktop's own")
        .expect("the dotfile must be writable");

    let removed = UnixFs::new()
        .remove_emptied_folder(root.path(), Some(&id), &below("album"))
        .await
        .expect("a folder that holds something is an answer, not a refusal");

    assert!(!removed);
    assert!(root.path().join("album/.DS_Store").is_file());
}

#[tokio::test]
async fn a_folder_that_is_not_there_is_nothing_to_remove() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());

    let removed = UnixFs::new()
        .remove_emptied_folder(root.path(), Some(&id), &below("album/sub"))
        .await
        .expect("a folder that is gone is an answer, not a refusal");

    assert!(!removed);
}

#[tokio::test]
async fn a_symbolic_link_at_the_name_is_neither_followed_nor_removed() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let elsewhere = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    let empty = elsewhere.path().join("empty");
    std::fs::create_dir(&empty).expect("the folder must be creatable");
    symlink(&empty, root.path().join("album")).expect("a link must be creatable");

    let refused = UnixFs::new()
        .remove_emptied_folder(root.path(), Some(&id), &below("album"))
        .await
        .expect_err("a link at the name is not a folder of the root");

    assert!(
        matches!(refused, DescentError::Blocked { .. }),
        "got {refused:?}"
    );
    assert!(root
        .path()
        .join("album")
        .symlink_metadata()
        .expect("the link is still there")
        .file_type()
        .is_symlink());
    assert!(empty.is_dir(), "and the folder past it is untouched");
}

#[tokio::test]
async fn a_root_that_is_not_the_registered_one_removes_nothing() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    register_root(root.path());
    std::fs::create_dir(root.path().join("album")).expect("the folder must be creatable");

    let refused = UnixFs::new()
        .remove_emptied_folder(
            root.path(),
            Some(&RootMarkerId::from_bytes([0x55; RootMarkerId::BYTE_LEN])),
            &below("album"),
        )
        .await
        .expect_err("a root carrying another identity must be refused");

    assert!(
        matches!(
            refused,
            DescentError::Refused {
                reason: RootRefused::MarkerMismatch,
                ..
            }
        ),
        "got {refused:?}"
    );
    assert!(root.path().join("album").is_dir());
}
