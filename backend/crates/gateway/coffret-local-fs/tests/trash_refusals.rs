//! What moving a departed file to the trash refuses before the trash is asked
//! at all (spec: EP-13, EP-15).
//!
//! Every case here is one the move turns away, and that is deliberate: a case
//! that let the move through would put a file into the trash of the desktop the
//! tests run on. What the move does once it is through is the platform trash's,
//! and what a sync does with either answer is held to the in-memory fake by the
//! sync conformance suite. These are the refusals this gateway owns — a root that
//! does not vouch for itself, a folder on the way down that is a symbolic link,
//! and a name that is not a regular file — and each asserts that the file is
//! exactly where it was.

#![cfg(unix)]

use std::os::unix::fs::symlink;
use std::path::Path;

use coffret_local_fs::UnixFs;
use coffret_model::EntryPath;
use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::root_marker::{self, MANAGEMENT_AREA, MARKER_FILE};
use coffret_usecase::{
    DescentError, LocalOperation, LocalTrash, MappedRelativeLocation, RootRefused,
};

const HELD: &[u8] = b"a departed file";

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
async fn a_root_that_is_not_the_registered_one_is_refused_and_the_file_stays() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    register_root(root.path());
    let file = root.path().join("a.jpg");
    std::fs::write(&file, HELD).expect("the file must be writable");

    let refused = UnixFs::new()
        .move_to_trash(
            root.path(),
            Some(&RootMarkerId::from_bytes([0x55; RootMarkerId::BYTE_LEN])),
            &below("a.jpg"),
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
    assert_eq!(std::fs::read(&file).expect("the file is still there"), HELD);
}

#[tokio::test]
async fn a_mapping_with_no_expected_identity_moves_nothing() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    register_root(root.path());
    let file = root.path().join("a.jpg");
    std::fs::write(&file, HELD).expect("the file must be writable");

    let refused = UnixFs::new()
        .move_to_trash(root.path(), None, &below("a.jpg"))
        .await
        .expect_err("a mapping that expects nothing is a root nothing may be written into");

    assert!(
        matches!(
            refused,
            DescentError::Refused {
                reason: RootRefused::NoExpectedIdentity,
                ..
            }
        ),
        "got {refused:?}"
    );
    assert_eq!(std::fs::read(&file).expect("the file is still there"), HELD);
}

#[tokio::test]
async fn a_folder_on_the_way_that_is_a_symbolic_link_is_not_followed() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let elsewhere = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    let file = elsewhere.path().join("a.jpg");
    std::fs::write(&file, HELD).expect("the file must be writable");
    symlink(elsewhere.path(), root.path().join("link")).expect("a link must be creatable");

    let refused = UnixFs::new()
        .move_to_trash(root.path(), Some(&id), &below("link/a.jpg"))
        .await
        .expect_err("a link on the way down must not be followed");

    assert!(
        matches!(refused, DescentError::Blocked { .. }),
        "got {refused:?}"
    );
    assert_eq!(
        std::fs::read(&file).expect("the file past the link is untouched"),
        HELD
    );
}

#[tokio::test]
async fn a_name_that_is_not_a_regular_file_is_refused() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    let target = root.path().join("target.jpg");
    std::fs::write(&target, HELD).expect("the file must be writable");
    symlink(&target, root.path().join("a.jpg")).expect("a link must be creatable");

    let refused = UnixFs::new()
        .move_to_trash(root.path(), Some(&id), &below("a.jpg"))
        .await
        .expect_err("a symbolic link at the name is not the file a sync found");

    assert!(
        matches!(&refused, DescentError::Io(io) if matches!(io.operation, LocalOperation::MovingToTrash)),
        "got {refused:?}"
    );
    assert!(root
        .path()
        .join("a.jpg")
        .symlink_metadata()
        .expect("the link is still there")
        .file_type()
        .is_symlink());
    assert_eq!(
        std::fs::read(&target).expect("its target is untouched"),
        HELD
    );
}
