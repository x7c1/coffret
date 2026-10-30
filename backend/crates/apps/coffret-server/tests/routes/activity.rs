//! The log of the work the server finished, as the work answer carries it
//! (spec: LA-12).

use axum::body::Body;
use coffret_server::SERVER_KEY_HEADER;

use crate::support::{asking, json as body_of, Served};

/// The entries of one work answer, newest first.
fn log(work: &serde_json::Value) -> &Vec<serde_json::Value> {
    work["log"]
        .as_array()
        .expect("the work answer carries the log")
}

// LA-2, LA-12: the log rides on the work answer, behind the same fence as
// every other route — a caller without the key learns nothing of what this
// device did. The one shown the key reads the log beside the work running now.
#[tokio::test]
async fn the_log_is_answered_beside_the_running_work_and_to_nobody_without_the_key() {
    let mut served = Served::library().await;

    // A file Storage would not give a reader is the first thing logged.
    served.halt_storage();
    let (status, _) = body_of(served.get("/api/file?path=albums/notes.txt").await).await;
    assert_eq!(status, 502);
    served.resume_storage();

    // And a fill held part way is the work running beside it.
    served.hold_storage();
    served.arm_fill("albums/2026");
    while served.held_reads() == 0 {
        tokio::task::yield_now().await;
    }

    // The request the explorer would send, with the key taken off it.
    let mut without_key = asking("GET", "/api/work");
    without_key
        .headers_mut()
        .expect("the request is well formed so far")
        .remove(SERVER_KEY_HEADER);
    let refused = served
        .send(
            without_key
                .body(Body::empty())
                .expect("a request with no body is well formed"),
        )
        .await;
    let (status, refusal) = body_of(refused).await;
    assert_eq!(status, 403, "{refusal}");
    assert_eq!(refusal["error"], "unauthorized");
    assert!(
        refusal.get("log").is_none(),
        "and nothing of the log: {refusal}"
    );

    let (status, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        work["fill"]["status"], "filling",
        "the running work: {work}"
    );
    assert_eq!(work["fill"]["folder"], "albums/2026");
    let entries = log(&work);
    assert_eq!(entries.len(), 1, "{work}");
    assert_eq!(entries[0]["kind"], "fetch");
    assert_eq!(entries[0]["path"], "albums/notes.txt");
    assert_eq!(entries[0]["refused"]["error"], "storage");
    assert_eq!(
        entries[0]["settled"], false,
        "a failure stands until it is settled"
    );
    assert!(entries[0]["at"]
        .as_str()
        .is_some_and(|at| at.ends_with('Z')));

    // The fill ends and is logged, newest first.
    served.release_storage();
    served.fill_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    let entries = log(&work);
    assert_eq!(entries.len(), 2, "{work}");
    assert_eq!(entries[0]["kind"], "fill");
    assert_eq!(entries[0]["run"]["folder"], "albums/2026");
    assert_eq!(entries[0]["run"]["status"], "done");
    assert_eq!(entries[1]["kind"], "fetch");

    // The file asked for again and answered settles the refusal before it.
    let fetched = served.get("/api/file?path=albums/notes.txt").await;
    assert_eq!(fetched.status(), 200);
    served.fill_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    let fetch = log(&work)
        .iter()
        .find(|entry| entry["kind"] == "fetch")
        .expect("the refused fetch is still in the log");
    assert_eq!(fetch["settled"], true, "{work}");

    // And the log is the process's own: a server started afterwards answers
    // with an empty one.
    served.restart();
    served.start_up().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert!(log(&work).is_empty(), "{work}");
}

// LA-12: a run that stopped is logged as a failure, and a later run of the same
// work settles it — here a sync Storage stopped, and the one after it.
#[tokio::test]
async fn a_sync_that_stopped_is_logged_until_the_next_sync_settles_it() {
    let served = Served::library().await;
    served.plant_locally("albums/added.jpg", b"added");

    served.halt_storage();
    assert_eq!(served.post("/api/sync").await.status(), 202);
    served.sync_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    let entries = log(&work);
    assert_eq!(entries.len(), 1, "{work}");
    assert_eq!(entries[0]["kind"], "sync");
    assert_eq!(entries[0]["run"]["status"], "stopped");
    assert_eq!(entries[0]["settled"], false);

    served.resume_storage();
    assert_eq!(served.post("/api/sync").await.status(), 202);
    served.sync_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    let entries = log(&work);
    assert_eq!(entries.len(), 2, "{work}");
    assert_eq!(entries[0]["run"]["status"], "done");
    assert_eq!(entries[0]["run"]["added"], 1);
    assert_eq!(entries[1]["run"]["status"], "stopped");
    assert_eq!(entries[1]["settled"], true, "the retry settled it");
}

// LA-12: not every refusal of a file is logged. A peek is nobody opening
// anything, an Entry the catalog does not hold is the reader holding a row the
// Library no longer has, and a locked Library is said once with its unlock —
// none of them is a failure the log would offer anything about.
#[tokio::test]
async fn a_peek_a_missing_entry_and_a_locked_library_are_not_logged() {
    let served = Served::library().await;

    let (status, refusal) = body_of(
        served
            .get("/api/file?path=albums/2026/spring.jpg&peek=true")
            .await,
    )
    .await;
    assert_eq!(status, 404, "{refusal}");

    let (status, refusal) = body_of(served.get("/api/file?path=albums/nothing.jpg").await).await;
    assert_eq!(status, 404);
    assert_eq!(refusal["error"], "no_such_entry");

    served.lock();
    let (status, refusal) = body_of(served.get("/api/file?path=albums/notes.txt").await).await;
    assert_eq!(status, 423);
    assert_eq!(refusal["error"], "locked");

    let (status, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(status, 200);
    assert!(log(&work).is_empty(), "{work}");
}
