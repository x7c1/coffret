//! What the sync a drop arms walks: the folders the drop wrote into, and
//! nothing else — while every other sync walks every mapped folder
//! (spec: EP-10, LA-34).
//!
//! A file put by hand into another folder waits for a sync that covers it, and
//! a file this device placed and lost out there is not read as deleted by a
//! drop's sync, nor one whose Entry left the Library as departed. The retry,
//! `POST /api/sync`, covers them all, and one armed while a drop's sync waits
//! for its turn is taken into that sync rather than run after it.

use std::path::{Path, PathBuf};

use coffret_logging::testing::CapturedLogs;
use serde_json::{json, Value};

use crate::support::{json as body_of, listing_of, states, sync, Served};

/// A folder of the case's own, mapped to the Library's `scans` component
/// through the route the explorer maps one by (spec: EP-9).
async fn mapped_scans(served: &Served) -> (tempfile::TempDir, PathBuf) {
    let made = tempfile::tempdir().expect("a temporary directory must be available");
    let root = made
        .path()
        .canonicalize()
        .expect("a temporary directory resolves");
    let (status, mapped) = body_of(
        served
            .post_json(
                "/api/map",
                &json!({
                    "local_root": root.to_str().expect("a temporary path is text"),
                    "prefix": "scans",
                }),
            )
            .await,
    )
    .await;
    assert_eq!(status, 200, "{mapped}");
    (made, root)
}

/// Puts a file into a mapped folder by hand, as a person copying one there
/// would.
fn put_by_hand(root: &Path, relative: &str, content: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("a file stands in a folder"))
        .expect("the folder can be made");
    std::fs::write(&path, content).expect("the file can be written");
}

/// The state the listing of `folder` gives `name`.
async fn state_of(served: &Served, folder: &str, name: &str) -> String {
    let listing = listing_of(served, folder).await;
    states(&listing)
        .into_iter()
        .find(|(listed, _)| listed == name)
        .unwrap_or_else(|| panic!("{folder}/{name} is listed: {listing}"))
        .1
}

/// The surfaced kind of each finding the sync on record carries, by path, in
/// path order.
fn surfaced(work: &Value) -> Vec<(String, String)> {
    let mut found: Vec<_> = sync(work)["findings"]
        .as_array()
        .expect("a sync carries findings")
        .iter()
        .map(|finding| {
            (
                finding["path"].as_str().unwrap_or_default().to_owned(),
                finding["surfaced"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    found.sort();
    found
}

// A drop's sync carries in what was dropped and leaves a file put by hand into
// another mapped folder where it is, not in the Library; the retry, which
// walks every mapped folder, carries that one in.
#[tokio::test]
async fn a_drops_sync_leaves_a_new_file_in_another_folder_for_a_sync_of_every_folder() {
    let served = Served::library().await;
    let (_made, scans) = mapped_scans(&served).await;
    put_by_hand(&scans, "vol-1/page-001.jpg", b"put there by hand");
    served.plant_locally("books/beside.png", b"beside the Library's book");

    let (status, _) = body_of(
        served
            .upload("albums", &[("one.jpg", b"one"), ("two.jpg", b"two")])
            .await,
    )
    .await;
    assert_eq!(status, 200);
    served.sync_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(sync(&work)["status"], "done", "{work}");
    assert_eq!(sync(&work)["added"], 2, "the dropped files alone: {work}");
    assert_eq!(state_of(&served, "albums", "one.jpg").await, "present");
    assert_eq!(state_of(&served, "albums", "two.jpg").await, "present");
    assert_eq!(
        state_of(&served, "scans/vol-1", "page-001.jpg").await,
        "added",
        "the file in the other mapped folder is not the drop's to carry in",
    );
    assert_eq!(state_of(&served, "books", "beside.png").await, "added");

    assert_eq!(served.post("/api/sync").await.status(), 202);
    served.sync_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(sync(&work)["status"], "done", "{work}");
    assert_eq!(sync(&work)["added"], 2, "what the drop's sync left: {work}");
    assert_eq!(
        state_of(&served, "scans/vol-1", "page-001.jpg").await,
        "present"
    );
    assert_eq!(state_of(&served, "books", "beside.png").await, "present");
}

// A drop's sync reads nothing outside the folders it walks as deleted or
// departed: a file this device placed and lost, and one whose Entry another
// device deleted, are left as they are — and the sync of every folder after
// it reports both.
#[tokio::test]
async fn a_drops_sync_reads_nothing_outside_its_folders_as_deleted_or_departed() {
    let served = Served::library().await;
    served
        .upload("books", &[("gone.png", b"gone"), ("kept.png", b"kept")])
        .await;
    served.sync_idle().await;

    std::fs::remove_file(served.local_path("books/gone.png"))
        .expect("the synced file is on the disk to remove");
    std::fs::write(
        served.local_path("books/kept.png"),
        b"kept, and edited since",
    )
    .expect("the synced file can be edited");
    served.delete_elsewhere("books/kept.png").await;

    served.upload("albums", &[("late.jpg", b"late")]).await;
    served.sync_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(sync(&work)["status"], "done", "{work}");
    assert_eq!(sync(&work)["added"], 1, "{work}");
    assert_eq!(
        sync(&work)["findings"],
        json!([]),
        "nothing outside the drop's folder is a finding",
    );
    assert!(
        served.holds("books/kept.png"),
        "the departed file is not looked at, let alone moved",
    );

    assert_eq!(served.post("/api/sync").await.status(), 202);
    served.sync_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(sync(&work)["status"], "done", "{work}");
    assert_eq!(
        surfaced(&work),
        [
            ("books/gone.png".to_owned(), "DeletedLocally".to_owned()),
            ("books/kept.png".to_owned(), "KeptEdited".to_owned()),
        ],
        "{work}",
    );
}

// The retry asked for while a drop's sync waits for its turn is taken into
// that sync: one run, which walks every mapped folder and so carries in both
// the drop and a file put by hand elsewhere.
#[tokio::test]
async fn a_sync_of_every_folder_armed_while_a_drops_sync_waits_runs_once_for_both() {
    let served = Served::library().await;
    served.plant_locally("books/beside.png", b"put there by hand");
    let logs = CapturedLogs::capture();
    served.hold_storage();

    let (status, _) = body_of(served.post("/api/delete?entry=albums/cover.png").await).await;
    assert_eq!(status, 202);
    while served.held_reads() == 0 {
        tokio::task::yield_now().await;
    }
    served.upload("albums", &[("late.jpg", b"late")]).await;
    served.until_queued(1).await;
    assert_eq!(served.post("/api/sync").await.status(), 202);

    served.release_storage();
    served.delete_idle().await;
    served.sync_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(sync(&work)["status"], "done", "{work}");
    assert_eq!(
        sync(&work)["added"],
        2,
        "the drop and the file beside: {work}"
    );
    assert_eq!(
        sync(&work)["run"],
        1,
        "the first run, and the only one: {work}"
    );
    let syncs = logs
        .events()
        .into_iter()
        .filter(|event| event.message() == "the mapped folders were carried into the Library")
        .count();
    assert_eq!(syncs, 1, "one sync ran");
    assert_eq!(state_of(&served, "books", "beside.png").await, "present");
}
