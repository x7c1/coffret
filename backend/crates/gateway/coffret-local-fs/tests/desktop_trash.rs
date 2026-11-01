//! That the desktop's own trash takes a file a discard lets through
//! (spec: EP-15).
//!
//! The one case in the workspace that reaches the trash of the desktop the
//! tests run on, and so the one that runs only when asked: it puts a temporary
//! file into the person's own trash, and a sandbox or a runner without a
//! usable trash refuses it. `make test-desktop-trash` runs it. Every other case
//! that discards does so through a test trash or is refused before the trash is
//! asked (`trash_refusals.rs`).

#![cfg(unix)]

use std::error::Error;
use std::path::Path;

use coffret_local_fs::UnixFs;
use coffret_model::EntryPath;
use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::root_marker::{self, MANAGEMENT_AREA, MARKER_FILE};
use coffret_usecase::{LocalTrash, MappedRelativeLocation};

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

/// `error` and every link beneath it, so that a trash that refused is read in
/// the operating system's own words rather than only as the discard that
/// failed.
fn every_link(error: &dyn Error) -> String {
    let mut rendered = error.to_string();
    let mut cause = error.source();
    while let Some(link) = cause {
        rendered.push_str(": ");
        rendered.push_str(&link.to_string());
        cause = link.source();
    }
    rendered
}

#[tokio::test]
#[ignore = "puts a temporary file into the desktop's own trash; run with `make test-desktop-trash`"]
async fn the_desktop_trash_takes_a_discarded_file() {
    let root = tempfile::tempdir().expect("a temporary directory must be available");
    let id = register_root(root.path());
    let name = "coffret-desktop-trash-check.txt";
    let file = root.path().join(name);
    std::fs::write(
        &file,
        b"put here by coffret's desktop trash check; safe to delete",
    )
    .expect("the file must be writable");
    let relative = MappedRelativeLocation::from_entry_path(
        &EntryPath::parse(name).expect("the case names a valid Entry Path"),
    );

    if let Err(refused) = UnixFs::new()
        .discard(root.path(), Some(&id), &relative)
        .await
    {
        panic!(
            "the desktop's trash refused the file: {}\n{refused:?}",
            every_link(&refused),
        );
    }

    assert!(
        !file.exists(),
        "the file left the mapped folder for the trash"
    );
}
