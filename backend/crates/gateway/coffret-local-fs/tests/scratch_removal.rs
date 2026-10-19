//! What removing a scratch a local writer left behind does against a real
//! directory (spec: OC-8, EP-11).
//!
//! A scratch nobody holds goes; one a writer is still writing — through a
//! handle of its own, which is how a writer in another process looks from here
//! — stays, because the writer holds its lock; and a symbolic link at the name
//! is neither followed nor removed.

#![cfg(unix)]

use std::os::unix::fs::symlink;
use std::path::Path;

use coffret_local_fs::UnixFs;
use coffret_model::EntryPath;
use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::root_marker::{self, MANAGEMENT_AREA, MARKER_FILE};
use coffret_usecase::{scratch, DescentError, Destinations, LocalTrash, MappedRelativeLocation};

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
async fn a_scratch_nobody_holds_is_removed() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    let name = scratch::incoming_name().as_str().to_owned();
    std::fs::create_dir(root.path().join("album")).expect("the folder must be creatable");
    std::fs::write(root.path().join("album").join(&name), b"half a file")
        .expect("the scratch must be writable");

    let fs = UnixFs::new();
    let removed = fs
        .remove_scratch(root.path(), Some(&id), &below(&format!("album/{name}")))
        .await
        .expect("a scratch below a vouched root must be removable");
    assert!(removed);
    assert!(!root.path().join("album").join(&name).exists());

    let again = fs
        .remove_scratch(root.path(), Some(&id), &below(&format!("album/{name}")))
        .await
        .expect("one already gone is the outcome sought (spec: OC-8)");
    assert!(!again);
}

#[tokio::test]
async fn a_scratch_a_writer_still_holds_stays() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    let fs = UnixFs::new();
    let destination = fs
        .reach(root.path(), Some(&id), &["a.jpg".to_owned()])
        .await
        .expect("the destination must be reachable");
    // Not held in this process's set, which is what a writer in another
    // process is from here: only its lock says it is still going.
    let name = scratch::incoming_name().as_str().to_owned();
    let mut writing = destination
        .create(&name)
        .expect("the scratch must be creatable");
    writing
        .write(b"still arriving")
        .await
        .expect("the scratch must be writable");

    let removed = fs
        .remove_scratch(root.path(), Some(&id), &below(&name))
        .await
        .expect("a held scratch is an answer, not a refusal");
    assert!(!removed, "a writer that is still going keeps its scratch");
    assert!(root.path().join(&name).is_file());

    // Flushed, it is still held until it is renamed.
    let flushed = writing.flush().await.expect("the flush must succeed");
    let removed = fs
        .remove_scratch(root.path(), Some(&id), &below(&name))
        .await
        .expect("a held scratch is an answer, not a refusal");
    assert!(!removed);
    flushed.publish().expect("the rename must succeed");
    assert!(root.path().join("a.jpg").is_file());
}

#[tokio::test]
async fn a_symbolic_link_at_the_name_is_neither_followed_nor_removed() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let elsewhere = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    let target = elsewhere.path().join("theirs.jpg");
    std::fs::write(&target, b"the person's own").expect("the file must be writable");
    let name = scratch::incoming_name().as_str().to_owned();
    symlink(&target, root.path().join(&name)).expect("a link must be creatable");

    let refused = UnixFs::new()
        .remove_scratch(root.path(), Some(&id), &below(&name))
        .await
        .expect_err("a link at the name is not a scratch");

    assert!(
        matches!(refused, DescentError::Blocked { .. }),
        "got {refused:?}"
    );
    assert!(root
        .path()
        .join(&name)
        .symlink_metadata()
        .expect("the link is still there")
        .file_type()
        .is_symlink());
    assert!(target.is_file(), "and the file past it is untouched");
}
