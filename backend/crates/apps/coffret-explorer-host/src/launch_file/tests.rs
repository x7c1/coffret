use std::fs;
use std::net::SocketAddr;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::launch_dir::{judge, write_exclusively};
use super::*;
use crate::credentials::{Clock, Credentials};

/// A clock a case moves by hand.
struct ManualClock(Mutex<Instant>);

impl ManualClock {
    fn advance(&self, by: Duration) {
        *self.0.lock().unwrap() += by;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        *self.0.lock().unwrap()
    }
}

fn credentials() -> (Credentials, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock(Mutex::new(Instant::now())));
    let credentials = Credentials::new(clock.clone()).expect("the CSPRNG answers");
    (credentials, clock)
}

fn host() -> SocketAddr {
    "127.0.0.1:5173".parse().unwrap()
}

fn mode_of(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().mode() & 0o777
}

/// The ticket the page in `file` refreshes to.
fn ticket_in(file: &Path) -> String {
    let page = fs::read_to_string(file).unwrap();
    let (_, after) = page
        .split_once("#launch=")
        .expect("the page names a ticket");
    after
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

// LA-21: the directory is made owner-only, and the file in it is the owner's
// alone, with nothing left beside it under another name.
#[test]
fn a_launch_file_is_the_owners_alone() {
    let parent = tempfile::tempdir().unwrap();
    let dir = LaunchDir::prepare(parent.path().join("launch")).expect("the directory is made");
    assert_eq!(mode_of(dir.path()), DIRECTORY_MODE);

    let (credentials, _) = credentials();
    let file = dir
        .deliver("explorer.html", host(), &credentials)
        .expect("the file is written");
    assert_eq!(file.path(), dir.path().join("explorer.html"));
    assert_eq!(mode_of(file.path()), FILE_MODE);
    let names: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, ["explorer.html"], "no temporary file is left behind");

    // The page is the host's own address, with the ticket in its fragment.
    let page = fs::read_to_string(file.path()).unwrap();
    let ticket = ticket_in(file.path());
    assert_eq!(ticket.len(), 43);
    assert!(page.contains(&format!("http://127.0.0.1:5173/#launch={ticket}")));
    assert!(page.contains(&credentials.instance_id().to_string()));
}

// LA-21: the page refreshes and links, and nothing in it runs.
#[test]
fn a_launch_file_holds_nothing_that_runs() {
    let parent = tempfile::tempdir().unwrap();
    let dir = LaunchDir::prepare(parent.path().join("launch")).unwrap();
    let (credentials, _) = credentials();
    let file = dir.deliver("explorer.html", host(), &credentials).unwrap();
    let page = fs::read_to_string(file.path())
        .unwrap()
        .to_ascii_lowercase();

    assert!(page
        .contains("<meta http-equiv=\"refresh\" content=\"0; url=http://127.0.0.1:5173/#launch="));
    assert!(page.contains("<a href=\"http://127.0.0.1:5173/#launch="));
    for forbidden in [
        "<script",
        "javascript:",
        "<iframe",
        "<object",
        "<embed",
        "<form",
        "<img",
    ] {
        assert!(!page.contains(forbidden), "{forbidden} in {page}");
    }
    // No event handler attribute, which is script by another name.
    let handlers = page
        .split(|c: char| c.is_whitespace() || c == '<')
        .filter(|word| word.starts_with("on") && word.contains('='))
        .count();
    assert_eq!(handlers, 0, "{page}");
}

// LA-21: created exclusively, so a file already at the temporary name is
// neither written through nor renamed into place.
#[test]
fn a_launch_file_is_created_exclusively() {
    let dir = tempfile::tempdir().unwrap();
    let temporary = dir.path().join(".explorer.html.tmp");
    let path = dir.path().join("explorer.html");
    fs::write(&temporary, "somebody else's").unwrap();

    let refused = write_exclusively(&temporary, &path, b"a ticket");
    assert!(
        matches!(
            refused,
            Err(LaunchFileError::Io {
                operation: "creating the file",
                ..
            })
        ),
        "{refused:?}"
    );
    assert_eq!(fs::read_to_string(&temporary).unwrap(), "somebody else's");
    assert!(!path.exists());

    fs::remove_file(&temporary).unwrap();
    write_exclusively(&temporary, &path, b"a ticket").expect("a fresh name is written");
    assert_eq!(fs::read_to_string(&path).unwrap(), "a ticket");
    assert!(!temporary.exists());
}

// LA-21: a directory anybody else can enter, or another account's, is
// refused, and so is a link standing where the directory should be.
#[test]
fn a_directory_that_is_not_the_owners_alone_is_refused() {
    let parent = tempfile::tempdir().unwrap();
    let wide = parent.path().join("wide");
    fs::DirBuilder::new().mode(0o755).create(&wide).unwrap();
    fs::set_permissions(&wide, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        LaunchDir::prepare(&wide),
        Err(LaunchFileError::TooOpen { mode: 0o755 })
    ));

    let narrow = parent.path().join("narrow");
    let dir = LaunchDir::prepare(&narrow).expect("a fresh directory is the owner's");
    let (credentials, _) = credentials();
    fs::set_permissions(&narrow, fs::Permissions::from_mode(0o710)).unwrap();
    assert!(
        matches!(
            dir.deliver("explorer.html", host(), &credentials),
            Err(LaunchFileError::TooOpen { mode: 0o710 })
        ),
        "a directory widened after it was prepared is refused at the next write"
    );

    let link = parent.path().join("link");
    std::os::unix::fs::symlink(&narrow, &link).unwrap();
    assert!(matches!(
        LaunchDir::prepare(&link),
        Err(LaunchFileError::NotADirectory)
    ));

    let file = parent.path().join("file");
    fs::write(&file, "").unwrap();
    assert!(matches!(
        LaunchDir::prepare(&file),
        Err(LaunchFileError::NotADirectory)
    ));

    // Another account's directory cannot be made without being that account,
    // so the verdict is asked of the rule itself.
    assert!(matches!(
        judge(true, 1001, 0o40700, 1000),
        Err(LaunchFileError::ForeignOwner)
    ));
    assert!(judge(true, 1000, 0o40700, 1000).is_ok());
}

// LA-21: a launch file opens the host on its own loopback address and is named
// as one page.
#[test]
fn a_launch_file_is_one_page_for_the_loopback_host() {
    let parent = tempfile::tempdir().unwrap();
    let dir = LaunchDir::prepare(parent.path().join("launch")).unwrap();
    let (credentials, _) = credentials();
    for name in ["", ".explorer.html", "a/b.html", "explorer.txt", ".html"] {
        assert!(
            matches!(
                dir.deliver(name, host(), &credentials),
                Err(LaunchFileError::NotAName)
            ),
            "{name:?}"
        );
    }
    let elsewhere: SocketAddr = "0.0.0.0:5173".parse().unwrap();
    assert!(matches!(
        dir.deliver("explorer.html", elsewhere, &credentials),
        Err(LaunchFileError::NotLoopback(_))
    ));
}

// LA-17, LA-21: a file whose ticket was exchanged, or has expired, is spent,
// and is removed; one already gone is not an error.
#[test]
fn a_launch_file_is_spent_once_used_or_expired() {
    let parent = tempfile::tempdir().unwrap();
    let dir = LaunchDir::prepare(parent.path().join("launch")).unwrap();
    let (credentials, clock) = credentials();
    let gate = credentials.gate();

    let used = dir.deliver("used.html", host(), &credentials).unwrap();
    assert!(!used.is_spent(&credentials));
    gate.exchange(&ticket_in(used.path()))
        .expect("the ticket in the file exchanges");
    assert!(used.is_spent(&credentials));
    let path = used.path().to_path_buf();
    used.remove().expect("the file is removed");
    assert!(!path.exists());

    let expired = dir.deliver("expired.html", host(), &credentials).unwrap();
    clock.advance(Duration::from_secs(59));
    assert!(!expired.is_spent(&credentials));
    clock.advance(Duration::from_secs(1));
    assert!(expired.is_spent(&credentials));
    fs::remove_file(expired.path()).unwrap();
    expired
        .remove()
        .expect("a file already gone is not an error");
}

// LA-21: what a run before left in the directory — launch files, and the
// temporary names they were written under — is swept away, and nothing else.
#[test]
fn a_sweep_removes_what_an_earlier_run_left_and_nothing_else() {
    let parent = tempfile::tempdir().unwrap();
    let dir = LaunchDir::prepare(parent.path().join("launch")).unwrap();
    let (credentials, _) = credentials();
    dir.deliver("explorer-0.html", host(), &credentials)
        .unwrap();
    fs::write(dir.path().join(".explorer-1.html.4242.0.tmp"), "half").unwrap();
    fs::write(dir.path().join("notes.txt"), "somebody's").unwrap();
    fs::DirBuilder::new()
        .create(dir.path().join("kept.html"))
        .unwrap();

    assert_eq!(dir.sweep().expect("the directory is swept"), 2);
    let mut left: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    left.sort();
    assert_eq!(left, ["kept.html", "notes.txt"]);
    assert_eq!(dir.sweep().unwrap(), 0, "a second sweep finds nothing");
}

// A stale file a sweep could not remove is said to be one that was not
// removed, not one that was not written.
#[test]
fn a_sweep_that_cannot_remove_a_file_says_so() {
    let parent = tempfile::tempdir().unwrap();
    let dir = LaunchDir::prepare(parent.path().join("launch")).unwrap();
    let stale = dir.path().join("explorer-0.html");
    fs::write(&stale, "a spent ticket").unwrap();

    // A directory nobody may remove anything from, its owner included; still
    // the owner's alone.
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o500)).unwrap();
    let swept = dir.sweep();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();

    if !stale.exists() {
        // Run as an account the mode does not bind, which removed it anyway.
        return;
    }
    let refused = swept.expect_err("the stale file would not go");
    assert!(
        matches!(&refused, LaunchFileError::NotRemoved(cause) if cause.kind() == std::io::ErrorKind::PermissionDenied),
        "{refused:?}"
    );
    assert_eq!(
        refused.to_string(),
        "a launch file an earlier run left could not be removed"
    );
}
