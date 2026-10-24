//! A drop walked off this device's own disk, and held for its import.

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;

use coffret_logging::testing::CapturedLogs;
use coffret_usecase::{root_marker, scratch};
use tempfile::TempDir;

use super::*;

/// A folder of the case's own that things are dropped out of.
fn drops() -> TempDir {
    tempfile::tempdir().expect("a temporary directory must be available")
}

fn write(at: &Path, content: &[u8]) {
    fs::create_dir_all(at.parent().expect("a case writes below a folder"))
        .expect("the folders above a case's file are made");
    fs::write(at, content).expect("a case's file is written");
}

fn whole(path: &Path) -> String {
    path.to_str().expect("a temporary path is text").to_owned()
}

fn taken(dropped: &Dropped) -> Vec<&str> {
    dropped.files.iter().map(|row| row.path.as_str()).collect()
}

fn not_taken(dropped: &Dropped) -> Vec<(&str, &str)> {
    dropped
        .not_taken
        .iter()
        .map(|one| (one.path.as_str(), one.reason))
        .collect()
}

async fn walked(paths: &[String]) -> Dropped {
    Drops::new()
        .hold(walk_drop(paths).await)
        .expect("a handle is drawn")
}

// EP-8: a dropped folder is walked depth first — each folder before its next
// sibling — and its files are named relative to the folder it stood in.
#[tokio::test]
async fn a_nested_folder_is_taken_depth_first() {
    let dir = drops();
    for path in [
        "book/b.jpg",
        "book/a/2.jpg",
        "book/a/1.jpg",
        "book/c/d/x.jpg",
    ] {
        write(&dir.path().join(path), b"page");
    }

    let dropped = walked(&[whole(&dir.path().join("book"))]).await;

    assert_eq!(
        taken(&dropped),
        [
            "book/a/1.jpg",
            "book/a/2.jpg",
            "book/b.jpg",
            "book/c/d/x.jpg"
        ]
    );
    assert!(dropped.not_taken.is_empty(), "{:?}", dropped.not_taken);
    assert_eq!(dropped.files[0].size, 4);
}

// EP-2: an empty folder contributes nothing, and is no refusal.
#[tokio::test]
async fn an_empty_folder_contributes_nothing() {
    let dir = drops();
    fs::create_dir_all(dir.path().join("book/empty")).expect("a folder is made");
    write(&dir.path().join("book/page.jpg"), b"page");

    let dropped = walked(&[whole(&dir.path().join("book"))]).await;

    assert_eq!(taken(&dropped), ["book/page.jpg"]);
    assert!(dropped.not_taken.is_empty(), "{:?}", dropped.not_taken);
}

// EP-8: a symbolic link is neither followed nor taken, whether it points at a
// file or at a folder, and both are reported — under a dropped folder and
// dropped themselves.
#[tokio::test]
async fn a_symbolic_link_is_neither_followed_nor_taken_and_is_reported() {
    let dir = drops();
    write(&dir.path().join("elsewhere/secret.jpg"), b"not dropped");
    write(&dir.path().join("book/page.jpg"), b"page");
    symlink(
        dir.path().join("elsewhere/secret.jpg"),
        dir.path().join("book/to-file"),
    )
    .expect("a link is made");
    symlink(
        dir.path().join("elsewhere"),
        dir.path().join("book/to-folder"),
    )
    .expect("a link is made");
    symlink(dir.path().join("elsewhere"), dir.path().join("linked")).expect("a link is made");

    let dropped = walked(&[
        whole(&dir.path().join("book")),
        whole(&dir.path().join("linked")),
    ])
    .await;

    assert_eq!(taken(&dropped), ["book/page.jpg"]);
    assert_eq!(
        not_taken(&dropped),
        [
            ("book/to-file", "not_a_file_or_folder"),
            ("book/to-folder", "not_a_file_or_folder"),
            ("linked", "not_a_file_or_folder"),
        ]
    );
}

// EP-11, EP-14: the scratch prefix and the management area are refused by
// name, and nothing under the management area is walked.
#[tokio::test]
async fn a_scratch_name_and_the_management_area_are_refused() {
    let dir = drops();
    let scratch = scratch::incoming_name();
    write(&dir.path().join("book").join(scratch.as_str()), b"half");
    write(
        &dir.path()
            .join("book")
            .join(root_marker::MANAGEMENT_AREA)
            .join(root_marker::MARKER_FILE),
        b"marker",
    );
    write(&dir.path().join("book/page.jpg"), b"page");

    let dropped = walked(&[whole(&dir.path().join("book"))]).await;

    assert_eq!(taken(&dropped), ["book/page.jpg"]);
    let mut refused = not_taken(&dropped);
    refused.sort();
    let mut expected = [
        (format!("book/{}", root_marker::MANAGEMENT_AREA), "reserved"),
        (format!("book/{}", scratch.as_str()), "reserved"),
    ];
    expected.sort();
    assert_eq!(
        refused,
        expected
            .iter()
            .map(|(path, reason)| (path.as_str(), *reason))
            .collect::<Vec<_>>()
    );
}

// EP-17: what an operating system keeps beside the person's files travels to
// the page under words of its own, a file's apart from a folder's, so the page
// can say it apart from what was refused.
#[tokio::test]
async fn what_an_operating_system_keeps_travels_under_words_of_its_own() {
    let dir = drops();
    for path in [
        "book/page.jpg",
        "book/.DS_Store",
        "book/__MACOSX/._page.jpg",
    ] {
        write(&dir.path().join(path), b"page");
    }

    let dropped = walked(&[whole(&dir.path().join("book"))]).await;

    assert_eq!(taken(&dropped), ["book/page.jpg"]);
    assert_eq!(
        not_taken(&dropped),
        [
            ("book/.DS_Store", "system_file"),
            ("book/__MACOSX", "system_folder"),
        ]
    );
}

// A browser reads a large folder a hundred entries at a time; this listing is
// one call, and every entry of a folder past that is taken.
#[tokio::test]
async fn a_folder_of_more_than_a_hundred_entries_is_taken_whole() {
    let dir = drops();
    for page in 0..150 {
        write(
            &dir.path().join(format!("book/page-{page:03}.jpg")),
            b"page",
        );
    }

    let dropped = walked(&[whole(&dir.path().join("book"))]).await;

    assert_eq!(dropped.files.len(), 150);
    assert_eq!(dropped.files[0].path, "book/page-000.jpg");
    assert_eq!(dropped.files[149].path, "book/page-149.jpg");
}

// A file dropped on its own is taken under its own name, with its size and the
// modification time its filesystem reports.
#[tokio::test]
async fn a_single_dropped_file_is_taken_under_its_name() {
    let dir = drops();
    let photo = dir.path().join("photo.jpg");
    write(&photo, b"a photograph");
    let stamped = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_444_000_000);
    fs::File::options()
        .write(true)
        .open(&photo)
        .and_then(|file| file.set_modified(stamped))
        .expect("the file is stamped");

    let dropped = walked(&[whole(&photo)]).await;

    assert_eq!(
        dropped.files,
        [DroppedRow {
            path: "photo.jpg".to_owned(),
            size: 12,
            mtime_millis: 1_444_000_000_000,
        }]
    );
    assert!(dropped.handle.is_some());
}

// Nothing let go of disappears without a word: an empty path, one that is not
// there, and a folder the operating system will not list are each reported.
#[tokio::test]
async fn an_empty_missing_or_unreadable_path_is_reported() {
    let dir = drops();
    let locked = dir.path().join("locked");
    write(&locked.join("page.jpg"), b"page");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("the folder is locked");
    // Read back before it is asserted on: a process that may list a folder
    // whatever its mode — the superuser's — has nothing to say here.
    let listable = fs::read_dir(&locked).is_ok();

    let dropped = walked(&[
        String::new(),
        whole(&dir.path().join("gone.jpg")),
        whole(&locked),
    ])
    .await;
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755))
        .expect("the folder is unlocked for its removal");

    assert!(dropped.files.is_empty() || listable);
    assert_eq!(
        not_taken(&dropped)[..2],
        [("", "empty"), ("gone.jpg", "missing")]
    );
    if !listable {
        assert_eq!(not_taken(&dropped)[2], ("locked", "unreadable"));
    }
    assert_eq!(dropped.handle, None, "nothing taken, so nothing is held");
}

// A handle takes its drop once: the second time it is unknown.
#[tokio::test]
async fn a_handle_is_used_once() {
    let dir = drops();
    write(&dir.path().join("photo.jpg"), b"a photograph");
    let drops = Drops::new();
    let dropped = drops
        .hold(walk_drop(&[whole(&dir.path().join("photo.jpg"))]).await)
        .expect("a handle is drawn");
    let handle = dropped.handle.expect("a file was taken");

    let set = drops.take(&handle).expect("the drop is held");
    assert_eq!(set.len(), 1);
    assert_eq!(set.file(0).map(DroppedFile::path), Some("photo.jpg"));
    assert!(set.file(1).is_none(), "no file is held past the set");
    assert!(matches!(drops.take(&handle), Err(DropRefusal::Unknown)));
    assert!(matches!(
        drops.take("never-drawn"),
        Err(DropRefusal::Unknown)
    ));
}

// A drop nobody imported ends after its lifetime.
#[tokio::test(start_paused = true)]
async fn a_handle_expires() {
    let dir = drops();
    write(&dir.path().join("photo.jpg"), b"a photograph");
    let drops = Drops::new();
    let walk = walk_drop(&[whole(&dir.path().join("photo.jpg"))]).await;
    let first = drops.hold(walk.clone()).expect("a handle is drawn");
    let second = drops.hold(walk).expect("a handle is drawn");

    tokio::time::advance(DROP_LIFETIME - std::time::Duration::from_secs(1)).await;
    assert!(drops.take(second.handle.as_deref().expect("held")).is_ok());

    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    assert!(matches!(
        drops.take(first.handle.as_deref().expect("held")),
        Err(DropRefusal::Expired)
    ));
}

// At most a few drops are held, and one past them ends the oldest.
#[tokio::test]
async fn holding_past_the_cap_ends_the_oldest() {
    let dir = drops();
    write(&dir.path().join("photo.jpg"), b"a photograph");
    let walk = walk_drop(&[whole(&dir.path().join("photo.jpg"))]).await;
    let drops = Drops::new();
    let handles: Vec<String> = (0..=HELD_DROPS)
        .map(|_| {
            drops
                .hold(walk.clone())
                .expect("a handle is drawn")
                .handle
                .expect("a file was taken")
        })
        .collect();

    assert!(matches!(drops.take(&handles[0]), Err(DropRefusal::Unknown)));
    for handle in &handles[1..] {
        assert!(drops.take(handle).is_ok(), "the later ones are all held");
    }
    assert_eq!(
        handles
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        handles.len(),
        "every handle is its own",
    );
}

// What the page is told names nothing on this device: no whole path, only
// paths relative to the folder the dropped item stood in.
#[tokio::test]
async fn the_answer_carries_no_absolute_path() {
    let dir = drops();
    write(&dir.path().join("book/page.jpg"), b"page");
    symlink(dir.path().join("book"), dir.path().join("book/loop")).expect("a link is made");

    let dropped = walked(&[
        whole(&dir.path().join("book")),
        whole(&dir.path().join("gone")),
    ])
    .await;

    let answer = format!("{dropped:?}");
    assert!(
        !answer.contains(&whole(dir.path())),
        "the answer names the folder the drop came from: {answer}"
    );
    for row in &dropped.files {
        assert!(!row.path.starts_with('/'), "{}", row.path);
    }
    for one in &dropped.not_taken {
        assert!(!one.path.starts_with('/'), "{}", one.path);
    }
}

// EL-1: walking, holding and taking a drop records counts and refusal kinds,
// and never a path, a dropped file's name or the handle.
#[tokio::test]
async fn no_event_carries_a_path_a_name_or_the_handle() {
    let dir = drops();
    write(
        &dir.path().join("sentinel-book-5d1e/sentinel-page-77a0.jpg"),
        b"page",
    );
    let logs = CapturedLogs::capture();

    let drops = Drops::new();
    let dropped = drops
        .hold(
            walk_drop(&[
                whole(&dir.path().join("sentinel-book-5d1e")),
                whole(&dir.path().join("sentinel-gone-3c9b")),
            ])
            .await,
        )
        .expect("a handle is drawn");
    let handle = dropped.handle.expect("a file was taken");
    drops.take(&handle).expect("the drop is held");
    assert!(matches!(drops.take(&handle), Err(DropRefusal::Unknown)));

    assert!(
        !logs.events().is_empty(),
        "the walk and the take are recorded"
    );
    logs.assert_free_of(&[
        &whole(dir.path()),
        "sentinel-book-5d1e",
        "sentinel-page-77a0",
        "sentinel-gone-3c9b",
        &handle,
    ]);
}

// A file is read again through the confined reader, and one replaced by a link
// since the walk is refused rather than followed (spec: EP-8).
#[tokio::test]
async fn a_dropped_file_is_read_again_and_a_link_in_its_place_is_refused() {
    let dir = drops();
    write(&dir.path().join("book/page.jpg"), b"page");
    write(&dir.path().join("elsewhere.jpg"), b"not dropped");
    let drops = Drops::new();
    let dropped = drops
        .hold(walk_drop(&[whole(&dir.path().join("book"))]).await)
        .expect("a handle is drawn");
    let set = drops
        .take(dropped.handle.as_deref().expect("held"))
        .expect("the drop is held");
    let file = set.file(0).expect("the page is held");

    let mut reader = file.open().await.expect("the page opens");
    let mut buffer = [0u8; 16];
    let read = reader.read(&mut buffer).await.expect("the page reads");
    assert_eq!(&buffer[..read], b"page");

    fs::remove_file(dir.path().join("book/page.jpg")).expect("the page is removed");
    symlink(
        dir.path().join("elsewhere.jpg"),
        dir.path().join("book/page.jpg"),
    )
    .expect("a link is made in its place");
    assert!(file.open().await.is_err(), "a link is never followed");
}
