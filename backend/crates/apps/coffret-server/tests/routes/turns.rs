//! A sync, a freeze and a deletion taking turns at this device's pending rows.
//!
//! Each of the three owns those rows for the whole of a run, and the use case
//! refuses a second owner at once (spec: OC-2). The server runs the three on
//! workers of its own, so whichever is armed while another is running waits its
//! turn here rather than being that second owner: every run armed completes, in
//! the order it was armed, and none stops on the refusal. Every pair a person can
//! reach is a case here, except a deletion behind a book or behind another
//! deletion, which `delete.rs` holds. Each run is held inside Storage by the
//! case rather than slept on.

use coffret_logging::testing::CapturedLogs;
use serde_json::Value;

use crate::support::{deletion, freeze, json as body_of, sync, Served};

/// The three runs that ended, as `(operation, outcome)` in the order they
/// ended.
///
/// Read off the one event each run records as it finishes, which is the only
/// place the order of three independent workers is written down.
fn ended(logs: &CapturedLogs) -> Vec<(String, String)> {
    const ENDINGS: [&str; 3] = [
        "the mapped folders were carried into the Library",
        "a folder was packed into the Library",
        "a deletion from the Library ended",
    ];
    logs.events()
        .into_iter()
        .filter(|event| ENDINGS.contains(&event.message().as_str()))
        .map(|event| (event.field("operation"), event.field("outcome")))
        .collect()
}

/// `(operation, "done")` for each operation, in the order given.
fn all_done(operations: &[&str]) -> Vec<(String, String)> {
    operations
        .iter()
        .map(|operation| ((*operation).to_owned(), "done".to_owned()))
        .collect()
}

/// Waits until the run just armed is inside Storage, and so holds the rows.
async fn inside_storage(served: &Served) {
    while served.held_reads() == 0 {
        tokio::task::yield_now().await;
    }
}

/// Asserts that `run` is said to be armed and not yet moving: its flow's own
/// status, and no step — what the explorer reads as waiting for its turn.
fn waiting(run: &Value, status: &str) {
    assert_eq!(run["status"], status, "{run}");
    assert_eq!(run["step"], Value::Null, "{run}");
}

// A file dropped while a book is packed arms a sync, which waits for the book
// and then carries the file in — rather than stopping on its first line as a
// server that could not answer.
#[tokio::test]
async fn a_drop_while_a_book_is_packed_syncs_after_it() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    let logs = CapturedLogs::capture();
    served.hold_storage();

    served.arm_freeze("scans/vol-1");
    inside_storage(&served).await;
    let (status, _) = body_of(served.upload("albums", &[("late.jpg", b"late")]).await).await;
    assert_eq!(status, 200);
    served.until_queued(1).await;
    let (_, queued) = body_of(served.get("/api/work").await).await;
    waiting(sync(&queued), "syncing");

    served.release_storage();
    served.freeze_idle().await;
    served.sync_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(sync(&work)["added"], 1, "{work}");
    assert_eq!(ended(&logs), all_done(&["freeze", "sync"]));
}

// A book asked for while a sync is inside Storage waits for the sync.
#[tokio::test]
async fn a_book_armed_while_a_sync_runs_is_packed_after_it() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    let logs = CapturedLogs::capture();
    served.hold_storage();

    served.upload("albums", &[("late.jpg", b"late")]).await;
    inside_storage(&served).await;
    served.arm_freeze("scans/vol-1");
    served.until_queued(1).await;
    let (_, queued) = body_of(served.get("/api/work").await).await;
    waiting(freeze(&queued), "freezing");

    served.release_storage();
    served.sync_idle().await;
    served.freeze_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&work)["entries"], 1, "{work}");
    assert_eq!(ended(&logs), all_done(&["sync", "freeze"]));
}

// A drop and a book both armed while a deletion is running wait for it, and
// then run in the order they were armed.
#[tokio::test]
async fn a_sync_and_a_book_armed_while_a_deletion_runs_follow_it_in_turn() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    let logs = CapturedLogs::capture();
    served.hold_storage();

    let (status, _) = body_of(served.post("/api/delete?entry=albums/cover.png").await).await;
    assert_eq!(status, 202);
    inside_storage(&served).await;
    served.upload("albums", &[("late.jpg", b"late")]).await;
    served.until_queued(1).await;
    served.arm_freeze("scans/vol-1");
    served.until_queued(2).await;
    let (_, queued) = body_of(served.get("/api/work").await).await;
    waiting(sync(&queued), "syncing");
    waiting(freeze(&queued), "freezing");

    served.release_storage();
    served.delete_idle().await;
    served.sync_idle().await;
    served.freeze_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(deletion(&work)["entries"], 1, "{work}");
    // The drop, and the book's page beside it: a sync walks every mapping, and
    // it ran before the book did.
    assert_eq!(sync(&work)["added"], 2, "{work}");
    assert_eq!(freeze(&work)["entries"], 1, "{work}");
    assert_eq!(ended(&logs), all_done(&["delete", "sync", "freeze"]));
}

// A book armed while a deletion is itself waiting behind a sync runs after
// both: the queue is one queue, in the order the three were armed, and not one
// per flow.
#[tokio::test]
async fn a_book_armed_behind_a_waiting_deletion_runs_after_both() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    let logs = CapturedLogs::capture();
    served.hold_storage();

    served.upload("albums", &[("late.jpg", b"late")]).await;
    inside_storage(&served).await;
    let (status, _) = body_of(served.post("/api/delete?entry=albums/cover.png").await).await;
    assert_eq!(status, 202);
    served.until_queued(1).await;
    served.arm_freeze("scans/vol-1");
    served.until_queued(2).await;
    let (_, queued) = body_of(served.get("/api/work").await).await;
    waiting(deletion(&queued), "deleting");
    waiting(freeze(&queued), "freezing");

    served.release_storage();
    served.sync_idle().await;
    served.delete_idle().await;
    served.freeze_idle().await;

    assert_eq!(ended(&logs), all_done(&["sync", "delete", "freeze"]));
}

// DK-2, DK-4: the turn is waited for before the keys are taken. A drop armed
// while a book is packed holds no handle on the Library while it waits, so a
// lock that lands meanwhile stops it once its turn comes — in the words a run
// armed after the lock is stopped in — while the book, which took its handle
// before the lock, finishes.
#[tokio::test]
async fn a_run_waiting_its_turn_when_the_lock_lands_stops_on_the_lock() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    let logs = CapturedLogs::capture();
    served.hold_storage();

    served.arm_freeze("scans/vol-1");
    inside_storage(&served).await;
    served.upload("albums", &[("late.jpg", b"late")]).await;
    served.until_queued(1).await;
    served.lock();
    served.release_storage();
    served.freeze_idle().await;
    served.sync_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&work)["status"], "done", "{work}");
    assert_eq!(sync(&work)["status"], "stopped", "{work}");
    assert_eq!(sync(&work)["stopped"]["error"], "locked", "{work}");
    assert_eq!(
        ended(&logs),
        vec![
            ("freeze".to_owned(), "done".to_owned()),
            ("sync".to_owned(), "stopped".to_owned()),
        ],
    );
}
