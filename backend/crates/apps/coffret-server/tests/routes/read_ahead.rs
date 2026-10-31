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

use crate::support::{bytes, fill, json as body_of, Served};

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

/// The findings the work answer carries beside its runs: what reads that
/// armed no fill found (spec: LA-33).
fn found(work: &Value) -> &Vec<Value> {
    work["findings"]
        .as_array()
        .expect("the work answer carries the findings of reads")
}

/// One of `found`, without the number and the moment it was heard under.
fn said(finding: &Value) -> Value {
    let mut finding = finding.clone();
    let fields = finding.as_object_mut().expect("a finding is an object");
    assert!(
        fields.remove("id").is_some_and(|id| id.is_u64()),
        "numbered"
    );
    assert!(
        fields.remove("at").is_some_and(|at| at.is_string()),
        "stamped"
    );
    finding
}

const KEYRING_DEGRADED: &str = "the Library's Keyring is degraded: some of its replicas are \
                                missing or unreadable. Files still open, and the next run that \
                                writes to the Library repairs it";

const PARCEL_UNHELD: &str = "a part of a Container kept on this device was gone or damaged and \
                             was read from Storage again";

// LA-33, KL-15: what a read ahead's fetch read of the committed Keyring is
// said in the work answer's own findings, since the read armed no fill to
// carry it — and said there whether or not a fill ever follows. Read ahead
// again, it is said once.
#[tokio::test]
async fn what_a_read_ahead_found_of_the_keyring_is_said_in_the_work_answer() {
    let served = Served::library().await;
    served.degrade_the_keyring().await;

    let answer = served
        .get("/api/file?path=books/page-001.png&fill=false")
        .await;
    assert_eq!(answer.status(), 200, "the file still opens (spec: RV-2)");
    assert_eq!(
        served
            .get("/api/file?path=albums/cover.png&fill=false")
            .await
            .status(),
        200
    );
    served.fill_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["fill"], Value::Null, "nothing armed: {work}");
    assert!(log(&work).is_empty(), "and nothing logged: {work}");
    assert_eq!(
        found(&work).iter().map(said).collect::<Vec<_>>(),
        [json!({
            "path": null,
            "message": KEYRING_DEGRADED,
            "reason": "keyring_degraded",
        })],
        "said once, for both reads: {work}",
    );

    // A fill asked for afterwards is a run of its own, and says what its own
    // fetches find: the read ahead's finding is not handed to it.
    served.arm_fill("books");
    served.fill_idle().await;
    let (_, after) = body_of(served.get("/api/work").await).await;
    assert_eq!(fill(&after)["total"], 0, "nothing left to fetch: {after}");
    assert_eq!(fill(&after)["findings"], json!([]), "{after}");
    assert_eq!(found(&after).len(), 1, "still said: {after}");
}

// LA-33, PK-19, PK-21: a read ahead is one file fetched without a fill. Its
// bytes come back; the parcel it read that holds the start of a file it did
// not place is held as any read's is; no fill is recorded or started; and
// when that kept parcel is damaged, the read ahead of that file reads it from
// Storage again and the work answer says so in its own findings.
#[tokio::test]
async fn a_read_ahead_holds_its_parcels_and_says_what_it_found_not_held() {
    let served = Served::library().await;
    let logs = CapturedLogs::capture();
    let first = vec![0x41; 300 * 1024];
    served.commit_elsewhere("scans/vol1/000.jpg", &first).await;
    served
        .commit_elsewhere("scans/vol2/000.jpg", &[0x42; 1536 * 1024])
        .await;
    served
        .commit_elsewhere("scans/vol3/000.jpg", &[0x43; 100 * 1024])
        .await;
    served.pack_elsewhere("scans").await;
    served.start_up().await;

    let answer = served
        .get("/api/file?path=scans/vol1/000.jpg&fill=false")
        .await;
    assert_eq!(answer.status(), 200);
    assert_eq!(bytes(answer).await, first, "the bytes come back");
    strokes_finished(&logs, 1).await;
    served.fill_idle().await;
    let held = served.held_parcels().await;
    assert_eq!(
        held.len(),
        1,
        "the parcel the next book starts in is held: {held:?}"
    );
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        work["fill"],
        Value::Null,
        "no fill recorded or started: {work}"
    );
    assert!(log(&work).is_empty(), "{work}");
    assert!(found(&work).is_empty(), "nothing found yet: {work}");
    assert!(!served.holds("scans/vol2/000.jpg"));
    assert!(!served.holds("scans/vol3/000.jpg"), "nothing else placed");

    std::fs::write(&held[0].path, b"not the parcel any more")
        .expect("the kept parcel's file can be overwritten");
    let answer = served
        .get("/api/file?path=scans/vol2/000.jpg&fill=false")
        .await;
    assert_eq!(answer.status(), 200, "the page opens all the same");
    strokes_finished(&logs, 2).await;
    served.fill_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["fill"], Value::Null, "still no fill: {work}");
    assert_eq!(
        found(&work).iter().map(said).collect::<Vec<_>>(),
        [json!({
            "path": null,
            "message": PARCEL_UNHELD,
            "reason": "parcel_unheld",
        })],
        "{work}",
    );
}

// EP-9, LA-33: a read ahead of a file in a folder this device has not mapped
// is refused as a read of it is — there is no folder here to place it in —
// and sets nothing going.
#[tokio::test]
async fn a_read_ahead_of_an_unmapped_file_is_refused() {
    let served = Served::mapping_only("albums").await;

    let (status, refusal) = body_of(
        served
            .get("/api/file?path=books/page-001.png&fill=false")
            .await,
    )
    .await;
    assert_eq!(status, 409, "{refusal}");
    assert_eq!(refusal["error"], "declined");
    assert_eq!(refusal["reason"], "unmapped");

    served.fill_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["fill"], Value::Null, "{work}");
    assert!(
        log(&work).is_empty(),
        "a read ahead's refusal is not logged: {work}"
    );
    assert!(found(&work).is_empty(), "{work}");
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
