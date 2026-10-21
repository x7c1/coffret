//! The import: files the desktop app's window dropped by path, held under a
//! one-time handle and read off this device (spec: LA-26 to LA-29), landing
//! through the same writer a browser's drop goes through.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use coffret_device::DROP_LIFETIME;
use coffret_logging::testing::CapturedLogs;
use coffret_server::Allowance;
use serde_json::json;
use tempfile::TempDir;

use crate::support::{freeze, json as body_of, rows_of, written, Served};

/// A folder of the case's own, standing for wherever on this device the
/// person dropped files from.
struct Source {
    dir: TempDir,
}

impl Source {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("a temporary directory must be available"),
        }
    }

    /// Writes one file under the folder, stamped with `modified` seconds.
    fn file(&self, relative: &str, content: &[u8], modified: u64) -> PathBuf {
        let path = self.dir.path().join(relative);
        fs::create_dir_all(path.parent().expect("a case writes below a folder"))
            .expect("the folders above a case's file are made");
        fs::write(&path, content).expect("a case's file is written");
        fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|file| file.set_modified(UNIX_EPOCH + Duration::from_secs(modified)))
            .expect("the file is stamped");
        path
    }

    fn at(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }
}

// The whole of what an import is for: two photographs dropped by path land in
// the mapped folder, with the person's files' own modification times, and the
// sync the import armed carries them in.
#[tokio::test]
async fn files_dropped_by_path_land_with_their_own_times() {
    let served = Served::library().await;
    let source = Source::new();
    let held = source.file("held.jpg", b"held", 1_444_000_000);
    let moor = source.file("moor.jpg", b"moor", 1_500_000_000);

    let dropped = served.hold_drop(&[&held, &moor]).await;
    let (status, answer) = body_of(served.import_all("albums/2026", &dropped, false).await).await;

    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        written(&answer),
        ["albums/2026/held.jpg", "albums/2026/moor.jpg"]
    );
    assert_eq!(answer["refused"], json!([]));
    assert_eq!(
        fs::read(served.local_path("albums/2026/held.jpg")).expect("the file landed"),
        b"held"
    );
    let landed = fs::metadata(served.local_path("albums/2026/held.jpg"))
        .and_then(|metadata| metadata.modified())
        .expect("the landed file has a time");
    assert_eq!(landed, UNIX_EPOCH + Duration::from_secs(1_444_000_000));

    served.sync_idle().await;
    assert_eq!(
        served
            .entry("albums/2026/held.jpg")
            .await
            .mtime
            .as_unix_seconds(),
        1_444_000_000
    );
    assert_eq!(
        served
            .entry("albums/2026/moor.jpg")
            .await
            .mtime
            .as_unix_seconds(),
        1_500_000_000
    );
    assert!(
        fs::metadata(&held).is_ok(),
        "the person's own file is read, never moved"
    );
}

// FM-9, EP-11: an import reads the person's own file, so the birth time its
// filesystem reports is the one the Entry records — not the copy's.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn a_file_dropped_by_path_keeps_its_own_birth_time() {
    use std::os::macos::fs::FileTimesExt;

    let served = Served::library().await;
    let source = Source::new();
    let scanned = source.file("scanned.jpg", b"scanned", 1_444_000_000);
    let born = UNIX_EPOCH + Duration::from_secs(1_400_000_000);
    fs::File::options()
        .write(true)
        .open(&scanned)
        .and_then(|file| file.set_times(fs::FileTimes::new().set_created(born)))
        .expect("the file's birth time is set");

    let dropped = served.hold_drop(&[&scanned]).await;
    let (status, _) = body_of(served.import_all("albums", &dropped, false).await).await;
    assert_eq!(status, 200);
    served.sync_idle().await;

    let entry = served.entry("albums/scanned.jpg").await;
    assert_eq!(
        entry.btime.map(|btime| btime.as_unix_seconds()),
        Some(1_400_000_000),
        "the person's file's birth time, and not the moment the import wrote the copy",
    );
}

// EP-2, EP-11, EP-14: what the Library will not have is refused file by file,
// beside what landed — a name that is not an Entry Path, a whole path among
// them, and a name coffret keeps for itself.
#[tokio::test]
async fn written_and_refused_files_are_answered_side_by_side() {
    let served = Served::library().await;
    let source = Source::new();
    let one = source.file("one.jpg", b"one", 1_444_000_000);
    let two = source.file("two.jpg", b"two", 1_444_000_000);
    let three = source.file("three.jpg", b"three", 1_444_000_000);
    let dropped = served.hold_drop(&[&one, &two, &three]).await;

    let (status, answer) = body_of(
        served
            .import(&json!({
                "path": "albums",
                "handle": dropped.handle,
                "files": [
                    { "index": 0, "name": "one.jpg" },
                    { "index": 1, "name": "/etc/two.jpg" },
                    { "index": 2, "name": ".coffret/three.jpg" },
                ],
            }))
            .await,
    )
    .await;

    assert_eq!(status, 200, "{answer}");
    assert_eq!(written(&answer), ["albums/one.jpg"]);
    let refused = answer["refused"].as_array().expect("a list of refusals");
    assert_eq!(refused.len(), 2, "{answer}");
    assert_eq!(refused[0]["name"], "/etc/two.jpg");
    assert_eq!(refused[0]["error"], "bad_path");
    assert_eq!(refused[1]["name"], ".coffret/three.jpg");
    assert_eq!(refused[1]["error"], "declined");
    assert!(!Path::new("/etc/two.jpg").exists());
}

// PK-17: a book imported as a Pack is reserved before its first page lands, so
// a freeze stopped by Storage leaves the reservation for the next one.
#[tokio::test]
async fn a_book_is_reserved_before_its_first_page_and_packed() {
    let served = Served::library().await;
    let source = Source::new();
    for page in 1..=3 {
        source.file(
            &format!("vol-1/page-{page:03}.jpg"),
            b"a page",
            1_444_000_000,
        );
    }
    let dropped = served.hold_drop(&[&source.at("vol-1")]).await;
    served.halt_storage();

    let (status, answer) = body_of(served.import_all("scans", &dropped, true).await).await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        written(&answer),
        [
            "scans/vol-1/page-001.jpg",
            "scans/vol-1/page-002.jpg",
            "scans/vol-1/page-003.jpg",
        ]
    );
    served.freeze_idle().await;
    assert_eq!(
        served.reservations().await,
        1,
        "the book waits for its Pack rather than for a sync"
    );

    served.resume_storage();
    assert_eq!(
        served.post("/api/freeze?path=scans/vol-1").await.status(),
        202
    );
    served.freeze_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&work)["status"], "done", "{work}");
    assert_eq!(
        rows_of(&served, "scans/vol-1").await,
        (1..=3)
            .map(|page| (
                format!("page-{page:03}.jpg"),
                "present".to_owned(),
                "pack".to_owned()
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(served.reservations().await, 0);
}

// PK-17, LA-11: an import stopped before anything landed — this device has not
// the room — arms nothing and releases its reservation.
#[tokio::test]
async fn a_book_import_that_is_stopped_releases_its_reservation() {
    let served = Served::within(Allowance {
        space: |_| Ok(4),
        ..Allowance::generous()
    })
    .await;
    let source = Source::new();
    source.file("vol-1/page-001.jpg", b"a page too large", 1_444_000_000);
    let dropped = served.hold_drop(&[&source.at("vol-1")]).await;

    let (status, refusal) = body_of(served.import_all("scans", &dropped, true).await).await;

    assert_eq!(status, 507, "{refusal}");
    assert_eq!(refusal["written"], json!([]));
    assert_eq!(served.reservations().await, 0);
    assert!(!served.holds("scans/vol-1/page-001.jpg"));
}

// EP-11: a file that grew after it was dropped is refused part way through, and
// leaves neither a file nor the scratch its bytes were going to.
#[tokio::test]
async fn a_file_that_fails_mid_way_leaves_nothing_behind() {
    let served = Served::library().await;
    let source = Source::new();
    // Longer than one read, so the first stretch of it is written before the
    // second is found past the length the walk stated.
    let page = source.file("page.jpg", &[b'x'; 300 * 1024], 1_444_000_000);
    let dropped = served.hold_drop(&[&page]).await;
    fs::write(&page, vec![b'x'; 600 * 1024]).expect("the file grows after the drop");

    let (status, answer) = body_of(served.import_all("albums", &dropped, false).await).await;

    assert_eq!(status, 200, "{answer}");
    assert_eq!(written(&answer), Vec::<String>::new());
    assert_eq!(answer["refused"][0]["error"], "conflict");
    assert_eq!(
        served.folder_names("albums"),
        Vec::<String>::new(),
        "no file and no scratch is left in the folder",
    );
    let left: Vec<String> = fs::read_dir(served.local_path("albums"))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        left,
        Vec::<String>::new(),
        "nothing at all is left: {left:?}"
    );
}

// LA-26: a handle is used once, and one never drawn takes nothing.
#[tokio::test]
async fn an_unknown_or_used_handle_is_refused() {
    let served = Served::library().await;
    let source = Source::new();
    let page = source.file("page.jpg", b"a page", 1_444_000_000);
    let dropped = served.hold_drop(&[&page]).await;

    let (status, _) = body_of(served.import_all("albums", &dropped, false).await).await;
    assert_eq!(status, 200);
    let (status, refusal) = body_of(served.import_all("albums", &dropped, false).await).await;
    assert_eq!(status, 400, "a used handle: {refusal}");
    assert_eq!(refusal["error"], "bad_request");

    let (status, refusal) = body_of(
        served
            .import(&json!({ "path": "albums", "handle": "0".repeat(64), "files": [] }))
            .await,
    )
    .await;
    assert_eq!(status, 400, "an unknown handle: {refusal}");
}

// LA-26: a drop held past its lifetime takes nothing.
#[tokio::test]
async fn an_expired_handle_is_refused() {
    let served = Served::library().await;
    let source = Source::new();
    let page = source.file("page.jpg", b"a page", 1_444_000_000);
    let dropped = served.hold_drop(&[&page]).await;

    tokio::time::pause();
    tokio::time::advance(DROP_LIFETIME + Duration::from_secs(1)).await;
    let (status, refusal) = body_of(served.import_all("albums", &dropped, false).await).await;

    assert_eq!(status, 400, "{refusal}");
    assert!(!served.holds("albums/page.jpg"));
}

// LA-26, DK-2: an import onto a locked server is refused before the handle is
// read, so the drop is still held once the Library is unlocked again.
#[tokio::test]
async fn an_import_onto_a_locked_server_leaves_the_drop_held() {
    let served = Served::library().await;
    let source = Source::new();
    let page = source.file("page.jpg", b"a page", 1_444_000_000);
    let dropped = served.hold_drop(&[&page]).await;
    served.lock();

    let (status, refusal) = body_of(served.import_all("albums", &dropped, false).await).await;
    assert_eq!(status, 423, "{refusal}");
    assert_eq!(refusal["error"], "locked");
    assert!(!served.holds("albums/page.jpg"), "nothing landed");

    assert_eq!(served.unlock(), coffret_server::Unlocked::Now);
    let (status, answer) = body_of(served.import_all("albums", &dropped, false).await).await;
    assert_eq!(status, 200, "the drop was still held: {answer}");
    assert_eq!(written(&answer), ["albums/page.jpg"]);
}

// LA-28: one import names at most as many files as an upload may carry parts,
// and one past that is refused as a whole, before any of them lands.
#[tokio::test]
async fn an_import_of_more_files_than_one_gesture_carries_is_refused() {
    let served = Served::within(Allowance {
        parts: 2,
        ..Allowance::generous()
    })
    .await;
    let source = Source::new();
    let one = source.file("one.jpg", b"one", 1_444_000_000);
    let two = source.file("two.jpg", b"two", 1_444_000_000);
    let three = source.file("three.jpg", b"three", 1_444_000_000);
    let dropped = served.hold_drop(&[&one, &two, &three]).await;

    let (status, refusal) = body_of(served.import_all("albums", &dropped, false).await).await;

    assert_eq!(status, 413, "{refusal}");
    assert_eq!(refusal["error"], "bad_request");
    assert_eq!(
        served.folder_names("albums"),
        Vec::<String>::new(),
        "the count is asked before any file is read",
    );
}

// LA-27: a file is addressed by its index in the held set alone, and an index
// past it refuses the request before anything lands — using the handle up all
// the same.
#[tokio::test]
async fn an_index_outside_the_held_set_is_refused() {
    let served = Served::library().await;
    let source = Source::new();
    let page = source.file("page.jpg", b"a page", 1_444_000_000);
    let dropped = served.hold_drop(&[&page]).await;

    let (status, refusal) = body_of(
        served
            .import(&json!({
                "path": "albums",
                "handle": dropped.handle,
                "files": [
                    { "index": 0, "name": "page.jpg" },
                    { "index": 1, "name": "other.jpg" },
                ],
            }))
            .await,
    )
    .await;
    assert_eq!(status, 400, "{refusal}");
    assert!(!served.holds("albums/page.jpg"), "nothing landed");

    let (status, _) = body_of(served.import_all("albums", &dropped, false).await).await;
    assert_eq!(status, 400, "the handle was used by the refused request");
}

// LA-27: no path on this device is accepted over HTTP — a body carrying one in
// a field of its own is refused whole, and nothing is read from it.
#[tokio::test]
async fn a_body_naming_an_absolute_path_is_refused() {
    let served = Served::library().await;
    let source = Source::new();
    let page = source.file("page.jpg", b"a page", 1_444_000_000);
    let elsewhere = source.file("elsewhere.jpg", b"never dropped", 1_444_000_000);
    let dropped = served.hold_drop(&[&page]).await;

    let (status, refusal) = body_of(
        served
            .import(&json!({
                "path": "albums",
                "handle": dropped.handle,
                "files": [{ "index": 0, "name": "page.jpg", "source": elsewhere }],
            }))
            .await,
    )
    .await;
    assert_eq!(status, 400, "{refusal}");
    assert_eq!(refusal["error"], "bad_request");
    assert!(!served.holds("albums/page.jpg"));

    let (status, refusal) = body_of(
        served
            .import(&json!({
                "path": "albums",
                "handle": "0".repeat(64),
                "paths": [elsewhere],
                "files": [],
            }))
            .await,
    )
    .await;
    assert_eq!(status, 400, "{refusal}");
}

// EL-1: what an import records is counts and refusal kinds — never a path on
// this device, a dropped file's name, or the handle.
#[tokio::test]
async fn no_event_of_an_import_carries_a_path_a_name_or_the_handle() {
    let served = Served::library().await;
    let source = Source::new();
    let page = source.file(
        "sentinel-book-2f81/sentinel-page-c3d4.jpg",
        b"page",
        1_444_000_000,
    );
    let logs = CapturedLogs::capture();

    let dropped = served.hold_drop(&[&source.at("sentinel-book-2f81")]).await;
    let handle = dropped.handle.clone().expect("the page was taken");
    let (status, _) = body_of(served.import_all("albums", &dropped, false).await).await;
    assert_eq!(status, 200);
    let (status, _) = body_of(served.import_all("albums", &dropped, false).await).await;
    assert_eq!(status, 400);
    served.sync_idle().await;

    logs.assert_free_of(&[
        source.dir.path().to_str().expect("a text path"),
        page.to_str().expect("a text path"),
        "sentinel-book-2f81",
        "sentinel-page-c3d4",
        &handle,
    ]);
}
