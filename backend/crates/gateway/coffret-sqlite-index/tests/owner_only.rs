//! The mode an Index file is created at, whatever the process umask says.
//!
//! In a file of its own because the umask is the process's and not a thread's:
//! a case that loosened it beside others would loosen it under theirs too, and
//! each integration test file is a process of its own.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;

use coffret_sqlite_index::SqliteIndex;
use rustix::fs::Mode;
use rustix::process::umask;

// The catalog is plaintext and names Entry Paths, so a file that comes back
// after somebody deleted it comes back owner-only — not at the `0644` less
// whatever the umask removes, which under the permissive one set here would be
// a catalog every account on the machine could read.
#[test]
fn a_catalog_is_created_owner_only_under_a_permissive_umask() {
    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let file = directory.path().join("index.sqlite");

    let previous = umask(Mode::empty());
    let opened = SqliteIndex::open(&file);
    umask(previous);
    // Held open while the modes are read: the `-wal` and `-shm` beside the
    // catalog exist only while a connection does, and closing the last one
    // removes them.
    let _index = opened.expect("a new catalog is created where there is none");

    for name in ["index.sqlite", "index.sqlite-wal", "index.sqlite-shm"] {
        let path = directory.path().join(name);
        let metadata = std::fs::metadata(&path)
            .unwrap_or_else(|cause| panic!("{name} is there while the catalog is open: {cause}"));
        assert_eq!(
            metadata.permissions().mode() & 0o777,
            0o600,
            "{name} is its owner's alone",
        );
    }
}

// A file that is already there is the owner's to have set, and opening it
// neither rewrites nor re-modes it.
#[test]
fn an_existing_catalog_keeps_the_mode_it_has() {
    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let file = directory.path().join("index.sqlite");
    std::fs::write(&file, b"").expect("an empty file is written");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o640))
        .expect("the mode is set");

    SqliteIndex::open(&file).expect("an empty file opens as an empty catalog");

    let mode = std::fs::metadata(&file)
        .expect("the file is there")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o640);
}
