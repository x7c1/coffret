//! The read ahead: one file read, and fetched where this device does not have
//! it, without arming the fill of its folder (spec: LA-33).
//!
//! What the explorer reads it for is the first pages of the folder a reader is
//! about to step into, and the cover of a folder the focus is on. A fill of a
//! second folder supersedes the one running, so a read of those the ordinary
//! way would stop the fill of the folder still being read.

use coffret_logging::testing::CapturedLogs;
use serde_json::{json, Value};
use tracing::Level;

use crate::support::{fill, json as body_of, Served};

/// The entries of one work answer's log.
fn log(work: &Value) -> &Vec<Value> {
    work["log"]
        .as_array()
        .expect("the work answer carries the log")
}

// LA-33: a read ahead of a file this device does not have fetches it into
// place, and arms no fill: there is no run afterwards, and the file is here.
#[tokio::test]
async fn a_read_ahead_places_the_file_and_arms_no_fill() {
    let served = Served::library().await;

    let answer = served
        .get("/api/file?path=albums/cover.png&fill=false")
        .await;
    assert_eq!(answer.status(), 200);
    served.fill_idle().await;

    assert!(served.holds("albums/cover.png"), "the fetch placed it");
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["fill"], Value::Null, "no fill was armed: {work}");
    assert!(log(&work).is_empty(), "and nothing was logged: {work}");
    assert!(
        !served.holds("albums/notes.txt"),
        "the rest stayed where it was"
    );
}

// LA-33: the fill running when a read ahead lands goes on running. Two books
// in one Pack, the first read whole: the second's first page came with it, and
// the rest of the second has not. A fill of a third folder is held inside
// Storage; the read ahead of the second book's first page — answered from
// disk, in a folder this device does not wholly have, which a page read the
// ordinary way arms the fill of — leaves that fill where it was.
#[tokio::test]
async fn a_read_ahead_does_not_replace_the_fill_running() {
    let served = Served::library().await;
    for (index, (path, len)) in [
        ("scans/a/000.jpg", 100 * 1024),
        ("scans/b/000.jpg", 100 * 1024),
        ("scans/b/001.jpg", 1536 * 1024),
    ]
    .into_iter()
    .enumerate()
    {
        served
            .commit_elsewhere(path, &vec![0x40 + index as u8; len])
            .await;
    }
    served.pack_elsewhere("scans").await;
    served.start_up().await;
    served.arm_fill("scans/a");
    served.fill_idle().await;
    assert!(served.holds("scans/b/000.jpg"));
    assert!(!served.holds("scans/b/001.jpg"));

    served.hold_storage();
    served.arm_fill("albums");
    while served.held_reads() == 0 {
        tokio::task::yield_now().await;
    }
    let (_, running) = body_of(served.get("/api/work").await).await;
    let run = fill(&running)["run"].clone();
    assert_eq!(fill(&running)["folder"], "albums", "{running}");
    assert_eq!(fill(&running)["status"], "filling", "{running}");

    assert_eq!(
        served
            .get("/api/file?path=scans/b/000.jpg&fill=false")
            .await
            .status(),
        200,
    );
    let (_, after) = body_of(served.get("/api/work").await).await;
    assert_eq!(fill(&after)["folder"], "albums", "not replaced: {after}");
    assert_eq!(fill(&after)["run"], run, "the same run: {after}");
    assert_eq!(fill(&after)["status"], "filling", "{after}");
    assert_eq!(
        after["fill"]["waiting"],
        json!([]),
        "nothing armed behind it: {after}"
    );

    served.release_storage();
    served.fill_idle().await;
    let (_, done) = body_of(served.get("/api/work").await).await;
    assert_eq!(fill(&done)["folder"], "albums", "{done}");
    assert_eq!(fill(&done)["status"], "done", "it ran to its end: {done}");
    assert!(
        !served.holds("scans/b/001.jpg"),
        "and the folder read ahead of was not brought over",
    );
}

// LA-33, KL-15: what a read ahead's fetch read of the committed Keyring goes
// to the next fill published, since the read armed none to carry it. `books`
// holds the one file read ahead, so the fill asked for it afterwards fetches
// nothing and reads nothing — and its line still says what the read ahead
// stepped over, which is the only line a person who only opens files reads.
#[tokio::test]
async fn what_a_read_ahead_found_of_the_keyring_is_said_by_the_next_fill() {
    let served = Served::library().await;
    served.degrade_the_keyring().await;

    let answer = served
        .get("/api/file?path=books/page-001.png&fill=false")
        .await;
    assert_eq!(answer.status(), 200, "the file still opens (spec: RV-2)");
    served.fill_idle().await;
    let (_, before) = body_of(served.get("/api/work").await).await;
    assert_eq!(before["fill"], Value::Null, "nothing armed: {before}");

    served.arm_fill("books");
    served.fill_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    let fill = fill(&work);
    assert_eq!(fill["folder"], "books");
    assert_eq!(fill["status"], "done");
    assert_eq!(
        fill["total"], 0,
        "nothing left in the folder to fetch: {work}"
    );
    assert_eq!(
        fill["findings"],
        json!([{
            "path": null,
            "message": "the Library's Keyring is degraded: some of its replicas are missing or \
                        unreadable. Files still open, and the next run that writes to the Library \
                        repairs it",
            "reason": "keyring_degraded",
        }]),
        "{work}",
    );
}

// LA-33, PK-21: the kept parcels a read ahead's fetch found not held go to the
// next fill published, as what it read of the Keyring does. The first book's
// page leaves the parcel the second book's page lies in held; that parcel's
// file is damaged, and the read ahead of the second book's page reads it again
// from Storage. The fill asked for afterwards has nothing left to fetch, and
// its line still says it.
#[tokio::test]
async fn what_a_read_ahead_found_not_held_is_said_by_the_next_fill() {
    let served = Served::library().await;
    let logs = CapturedLogs::capture();
    served
        .commit_elsewhere("scans/vol1/000.jpg", &[0x41; 300 * 1024])
        .await;
    served
        .commit_elsewhere("scans/vol2/000.jpg", &[0x42; 1536 * 1024])
        .await;
    served.pack_elsewhere("scans").await;
    served.start_up().await;

    assert_eq!(
        served
            .get("/api/file?path=scans/vol1/000.jpg")
            .await
            .status(),
        200
    );
    strokes_finished(&logs, 1).await;
    served.fill_idle().await;
    let held = served.held_parcels().await;
    assert_eq!(
        held.len(),
        1,
        "a parcel is held for the second book: {held:?}"
    );
    std::fs::write(&held[0].path, b"not the parcel any more")
        .expect("the kept parcel's file can be overwritten");

    let answer = served
        .get("/api/file?path=scans/vol2/000.jpg&fill=false")
        .await;
    assert_eq!(answer.status(), 200, "the page opens all the same");
    strokes_finished(&logs, 2).await;
    served.fill_idle().await;
    let (_, before) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        fill(&before)["folder"],
        "scans/vol1",
        "nothing armed: {before}"
    );

    served.arm_fill("scans/vol2");
    served.fill_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    let fill = fill(&work);
    assert_eq!(fill["folder"], "scans/vol2");
    assert_eq!(fill["status"], "done");
    assert_eq!(
        fill["total"], 0,
        "nothing left in the folder to fetch: {work}"
    );
    assert_eq!(
        fill["findings"],
        json!([{
            "path": null,
            "message": "a part of a Container kept on this device was gone or damaged and was \
                        read from Storage again",
            "reason": "parcel_unheld",
        }]),
        "{work}",
    );
}

/// Waits for `strokes` partial fetches to have finished: the rest of a
/// stroke goes on after its reader was answered, and what it finds not held is
/// heard only once it is over.
async fn strokes_finished(logs: &CapturedLogs, strokes: usize) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let finished = logs
            .at(Level::INFO)
            .into_iter()
            .filter(|event| event.message() == "a partial fetch finished")
            .count();
        if finished >= strokes {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{strokes} partial fetches finish"
        );
        tokio::task::yield_now().await;
    }
}

// LA-12, LA-33: a read ahead that answers settles the refusal logged before it
// for the same file, as a read does: the file is here now, and a failure left
// standing would tell its reader otherwise.
#[tokio::test]
async fn a_read_ahead_answered_settles_the_refusal_before_it() {
    let served = Served::library().await;
    served.halt_storage();
    assert_ne!(
        served.get("/api/file?path=albums/cover.png").await.status(),
        200
    );
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(log(&work).len(), 1, "{work}");
    assert_eq!(log(&work)[0]["kind"], "fetch", "{work}");
    assert_eq!(log(&work)[0]["settled"], false, "{work}");

    served.resume_storage();
    let answer = served
        .get("/api/file?path=albums/cover.png&fill=false")
        .await;
    assert_eq!(answer.status(), 200);
    served.fill_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(log(&work).len(), 1, "nothing more was logged: {work}");
    assert_eq!(log(&work)[0]["path"], "albums/cover.png", "{work}");
    assert_eq!(log(&work)[0]["settled"], true, "{work}");
    assert_eq!(work["fill"], Value::Null, "and nothing was armed: {work}");
}

// LA-12, LA-33: a read ahead is the explorer guessing what will be opened
// next, and its refusal is not logged: the file is asked for again, and the
// refusal said, when somebody opens it.
#[tokio::test]
async fn a_refused_read_ahead_is_not_logged() {
    let served = Served::library().await;
    served.halt_storage();

    let answer = served
        .get("/api/file?path=albums/cover.png&fill=false")
        .await;
    assert_ne!(answer.status(), 200);

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert!(log(&work).is_empty(), "{work}");
    assert_eq!(work["fill"], Value::Null, "{work}");
}
