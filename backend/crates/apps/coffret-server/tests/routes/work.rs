//! The work answer as a whole: which process it came from, and what it says
//! before anybody has opened or dropped anything (spec: LA-12).

use serde_json::json;

use crate::support::{json as body_of, without_server, Served};

// A browser keeps things across answers that are only true of one process — the
// run of a fill or a sync whose line somebody read and put away, counted from 1
// by each flow — and a locked Library is opened by starting the server again
// (spec: DK-1), so a tab outliving a restart is an ordinary case (spec: LA-12).
// Without this the new process's first runs would be silently hidden by the old
// one's dismissals, and no comparison of run numbers can tell that apart from an
// answer that was in flight when the button was pressed.
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
        "one process answers under one name, or a page would forget what it \
         holds on every tick",
    );

    // The answers to the routes a button presses, and not the polling one
    // alone. A page reads the name out of these too — they are what a retry
    // hears back, and the run they name is the one the button just started — so
    // an answer here without it would have a tab throw away every dismissal it
    // holds on every press. All three of them, because all three are pressed:
    // the bar offers a second attempt at each of the flows, and the two that
    // take a folder offer one for every folder their queue lost besides.
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
        "and another server is another name, which is what ends the dismissals \
         the first one's runs were put away by",
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
            "fill": null,
            "sync": null,
            "freeze": null,
        })
    );
}
