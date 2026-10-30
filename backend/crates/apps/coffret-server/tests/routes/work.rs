//! The work answer as a whole: which process it came from, and what it says
//! before anybody has opened or dropped anything (spec: LA-12).

use serde_json::json;

use crate::support::{json as body_of, without_server, Served};

// The run numbers an answer carries are only true of one process — each flow
// counts them from 1 — and a server started from the command line is unlocked
// by starting it again (spec: DK-1), so a tab outliving a restart is an ordinary
// case (spec: LA-12). Without a name for the process, run 1 of the new server
// reads like run 1 of the old one.
#[tokio::test]
async fn every_answer_says_which_process_it_came_from() {
    let served = Served::library().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    let name = work["server"]
        .as_str()
        .expect("an work says which process answered it");
    assert!(!name.is_empty(), "and it is a name rather than nothing");

    let (_, again) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        again["server"], work["server"],
        "one process answers under one name, or a page would read every tick \
         as a new process",
    );

    // The answers to the routes a button presses, and not the polling one
    // alone: they are what a retry hears back, and the run they name is the one
    // the button just started. All three of them, because the Activity panel
    // offers Retry on each of the flows.
    let (status, armed) = body_of(served.post("/api/sync").await).await;
    assert_eq!(status, 202);
    assert_eq!(
        armed["server"], work["server"],
        "the answer to a press names the process like any other: {armed}",
    );
    served.sync_idle().await;

    let (status, armed) = body_of(served.post("/api/fill?path=albums").await).await;
    assert_eq!(status, 202);
    assert_eq!(
        armed["server"], work["server"],
        "and so does one that a folder's button reaches: {armed}",
    );
    served.fill_idle().await;

    let (status, armed) = body_of(served.post("/api/freeze?path=albums").await).await;
    assert_eq!(status, 202);
    assert_eq!(
        armed["server"], work["server"],
        "and so does the one a book's button reaches: {armed}",
    );
    served.freeze_idle().await;

    let other = Served::library().await;
    let (_, elsewhere) = body_of(other.get("/api/work").await).await;
    assert_ne!(
        elsewhere["server"], work["server"],
        "and another server is another name, so its runs are not read as the \
         first one's",
    );
}

// An explorer that has opened nothing and dropped nothing has nothing
// to be told about, and the route says so rather than inventing work
// nobody started (spec: LA-12).
#[tokio::test]
async fn nothing_is_happening_before_anything_is_opened_or_dropped() {
    let served = Served::library().await;

    let (status, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        without_server(&work),
        json!({
            "library": "unlocked",
            "catalog": { "state": "caught_up", "stopped": null },
            "kept_parcels": { "count": 0, "bytes": 0 },
            "turn": null,
            "fill": null,
            "sync": null,
            "reconnect": null,
            "freeze": null,
            "delete": null,
            "log": [],
        })
    );
}
