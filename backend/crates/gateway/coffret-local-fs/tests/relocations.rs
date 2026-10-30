//! What relocating a file to follow a rename of its Entry does against a real
//! directory (spec: EP-8, EP-10, EP-13).
//!
//! One rename between two folders entered without following a link, made
//! only through a root that vouches for itself: it makes the folders the new
//! name needs, never replaces what stands there, never renames through a
//! symbolic link, and an exchange swaps two files with neither ever replaced.

#![cfg(unix)]

use std::os::unix::fs::symlink;
use std::path::Path;

use coffret_local_fs::UnixFs;
use coffret_model::EntryPath;
use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::root_marker::{self, MANAGEMENT_AREA, MARKER_FILE};
use coffret_usecase::{
    DescentError, MappedName, MappedRelativeLocation, RelocateError, Relocations, RootRefused,
};

/// Writes the marker a registration would have written into `root`, and hands
/// back the identity its mapping records (spec: EP-13).
fn register_root(root: &Path) -> RootMarkerId {
    let id = RootMarkerId::from_bytes([0x2b; RootMarkerId::BYTE_LEN]);
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

/// One name below `root`, held against `id`.
fn named<'a>(
    root: &'a Path,
    id: &'a RootMarkerId,
    relative: &'a MappedRelativeLocation,
) -> MappedName<'a> {
    MappedName {
        root,
        expected: Some(id),
        relative,
    }
}

/// Relocates `from` below `root` to `to` below the same root.
async fn relocate(
    root: &Path,
    id: &RootMarkerId,
    from: &str,
    to: &str,
) -> Result<(), RelocateError> {
    let (from, to) = (below(from), below(to));
    UnixFs::new()
        .relocate(named(root, id, &from), named(root, id, &to))
        .await
}

/// Exchanges `one` and `other` below `root`.
async fn exchange(
    root: &Path,
    id: &RootMarkerId,
    one: &str,
    other: &str,
) -> Result<(), RelocateError> {
    let (one, other) = (below(one), below(other));
    UnixFs::new()
        .exchange(named(root, id, &one), named(root, id, &other))
        .await
}

/// A file in `root` at `relative`, holding `content`, with its folders.
fn plant(root: &Path, relative: &str, content: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("a planted file has a folder"))
        .expect("the folders must be creatable");
    std::fs::write(path, content).expect("the file must be writable");
}

#[tokio::test]
async fn a_file_is_relocated_to_its_new_path_and_the_folders_it_needs_are_made() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    plant(root.path(), "inbox/a.jpg", b"the bytes");
    let mtime = std::fs::metadata(root.path().join("inbox/a.jpg"))
        .and_then(|meta| meta.modified())
        .expect("the file has a modification time");

    relocate(root.path(), &id, "inbox/a.jpg", "albums/2026/a.jpg")
        .await
        .expect("a relocation below a vouched root must succeed");

    let relocated = root.path().join("albums/2026/a.jpg");
    assert_eq!(std::fs::read(&relocated).unwrap(), b"the bytes");
    assert_eq!(
        std::fs::metadata(&relocated)
            .and_then(|meta| meta.modified())
            .unwrap(),
        mtime,
        "a rename leaves the modification time as it was"
    );
    assert!(!root.path().join("inbox/a.jpg").exists());
}

#[tokio::test]
async fn a_file_standing_at_the_new_path_is_never_replaced() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    plant(root.path(), "a.jpg", b"ours");
    plant(root.path(), "b.jpg", b"theirs");

    let refused = relocate(root.path(), &id, "a.jpg", "b.jpg")
        .await
        .expect_err("a taken name must be refused");

    assert!(
        matches!(refused, RelocateError::Taken { .. }),
        "{refused:?}"
    );
    assert_eq!(std::fs::read(root.path().join("a.jpg")).unwrap(), b"ours");
    assert_eq!(std::fs::read(root.path().join("b.jpg")).unwrap(), b"theirs");
}

#[tokio::test]
async fn a_move_through_a_symbolic_link_is_refused_and_makes_nothing() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let outside = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    plant(root.path(), "a.jpg", b"ours");
    symlink(outside.path(), root.path().join("linked")).expect("a link must be creatable");

    let refused = relocate(root.path(), &id, "a.jpg", "linked/deeper/a.jpg")
        .await
        .expect_err("a link on the way must be refused");

    assert!(
        matches!(
            refused,
            RelocateError::Descent(DescentError::Blocked { .. })
        ),
        "{refused:?}"
    );
    assert!(root.path().join("a.jpg").is_file());
    assert_eq!(
        std::fs::read_dir(outside.path()).unwrap().count(),
        0,
        "nothing is made past the link"
    );
}

#[tokio::test]
async fn a_file_that_is_a_symbolic_link_is_not_moved() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    plant(root.path(), "target.jpg", b"ours");
    symlink(root.path().join("target.jpg"), root.path().join("a.jpg"))
        .expect("a link must be creatable");

    let refused = relocate(root.path(), &id, "a.jpg", "b.jpg")
        .await
        .expect_err("a link standing for the file must be refused");

    assert!(
        matches!(
            refused,
            RelocateError::Descent(DescentError::Blocked { .. })
        ),
        "{refused:?}"
    );
    assert!(!root.path().join("b.jpg").exists());
}

#[tokio::test]
async fn a_refused_relocation_removes_the_folders_it_made() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    plant(root.path(), "locked/a.jpg", b"ours");
    let locked = root.path().join("locked");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555))
        .expect("the folder's permissions must be settable");
    // A process the permission does not bind — one running as root — cannot
    // be refused this way, and has nothing to show here.
    if std::fs::write(locked.join("probe"), b"").is_ok() {
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let refused = relocate(root.path(), &id, "locked/a.jpg", "made/deeper/a.jpg").await;
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert!(
        matches!(refused, Err(RelocateError::Descent(DescentError::Io(_)))),
        "{refused:?}"
    );
    assert!(locked.join("a.jpg").is_file(), "the file stays");
    assert!(
        !root.path().join("made").exists(),
        "the folders made for the refused relocation are removed again"
    );
}

#[tokio::test]
async fn two_files_exchange_their_names() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    plant(root.path(), "a.jpg", b"first");
    plant(root.path(), "sub/b.jpg", b"second");

    exchange(root.path(), &id, "a.jpg", "sub/b.jpg")
        .await
        .expect("exchanging two files below a vouched root must succeed");

    assert_eq!(std::fs::read(root.path().join("a.jpg")).unwrap(), b"second");
    assert_eq!(
        std::fs::read(root.path().join("sub/b.jpg")).unwrap(),
        b"first"
    );
}

#[tokio::test]
async fn nothing_is_moved_through_a_root_that_does_not_vouch_for_itself() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    register_root(root.path());
    plant(root.path(), "a.jpg", b"ours");
    let other = RootMarkerId::from_bytes([0x3c; RootMarkerId::BYTE_LEN]);

    let refused = relocate(root.path(), &other, "a.jpg", "b.jpg")
        .await
        .expect_err("a root with another identity must be refused");

    assert!(
        matches!(
            refused,
            RelocateError::Descent(DescentError::Refused {
                reason: RootRefused::MarkerMismatch,
                ..
            })
        ),
        "{refused:?}"
    );
    assert!(root.path().join("a.jpg").is_file());
    assert!(!root.path().join("b.jpg").exists());
}

// EP-10: two mapped roots on one filesystem are one rename apart, each root
// vouching for itself.
#[tokio::test]
async fn a_file_is_relocated_from_one_root_to_another_on_the_same_filesystem() {
    let albums = tempfile::tempdir().expect("a temporary directory must be available");
    let books = tempfile::tempdir().expect("a temporary directory must be available");
    let albums_id = register_root(albums.path());
    let books_id = RootMarkerId::from_bytes([0x4d; RootMarkerId::BYTE_LEN]);
    let area = books.path().join(MANAGEMENT_AREA);
    std::fs::create_dir_all(&area).expect("making the management area must succeed");
    std::fs::write(area.join(MARKER_FILE), root_marker::spell(&books_id))
        .expect("writing the marker must succeed");
    plant(albums.path(), "a.jpg", b"the bytes");
    let (from, to) = (below("a.jpg"), below("vol-1/a.jpg"));

    UnixFs::new()
        .relocate(
            named(albums.path(), &albums_id, &from),
            named(books.path(), &books_id, &to),
        )
        .await
        .expect("a relocation between two vouched roots on one filesystem must succeed");

    assert_eq!(
        std::fs::read(books.path().join("vol-1/a.jpg")).unwrap(),
        b"the bytes"
    );
    assert!(!albums.path().join("a.jpg").exists());
}

// EP-13: the root a file is relocated into vouches for itself as well, and a
// refusal there moves nothing.
#[tokio::test]
async fn nothing_is_moved_into_a_root_that_does_not_vouch_for_itself() {
    let albums = tempfile::tempdir().expect("a temporary directory must be available");
    let books = tempfile::tempdir().expect("a temporary directory must be available");
    let albums_id = register_root(albums.path());
    register_root(books.path());
    let other = RootMarkerId::from_bytes([0x3c; RootMarkerId::BYTE_LEN]);
    plant(albums.path(), "a.jpg", b"ours");
    let (from, to) = (below("a.jpg"), below("a.jpg"));

    let refused = UnixFs::new()
        .relocate(
            named(albums.path(), &albums_id, &from),
            named(books.path(), &other, &to),
        )
        .await
        .expect_err("a root with another identity must be refused");

    assert!(
        matches!(
            refused,
            RelocateError::Descent(DescentError::Refused {
                reason: RootRefused::MarkerMismatch,
                ..
            })
        ),
        "{refused:?}"
    );
    assert!(albums.path().join("a.jpg").is_file());
    assert!(!books.path().join("a.jpg").exists());
}
