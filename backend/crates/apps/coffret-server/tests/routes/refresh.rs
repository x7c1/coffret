//! Catching the catalog up with the Library, at startup and when somebody
//! presses refresh (spec: CK-9).

use std::time::Duration;

use serde_json::json;

use crate::support::{files, json as body_of, listing_of, states, Served};

// The first thing a server does, and the whole of what makes a joined device
// worth serving: the Journal is replayed into the catalog, and the Library is on
// the screen without anything having been typed at a terminal (spec: CK-9).
//
// And nothing else happens. No Container is read and no file is placed: every
// row arrives `remote`, which is exactly what a device that has the catalog and
// none of the files is (spec: EP-10).
#[tokio::test]
async fn starting_up_catches_the_catalog_up_with_the_library() {
    let served = Served::joined().await;

    let (status, before) = body_of(served.get("/api/folders").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        before,
        json!({ "folders": [] }),
        "a device that has just joined has replayed nothing",
    );

    served.start_up().await;

    let (_, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(
        listed,
        json!({ "folders": ["albums", "albums/2026", "books"] })
    );
    let (_, listing) = body_of(served.get("/api/list?path=albums").await).await;
    assert!(
        states(&listing).iter().all(|(_, state)| state == "remote"),
        "the catalog knows them and this device has none of them: {listing}",
    );
    assert_eq!(
        served.ranged_reads(),
        0,
        "a catch-up opens no Container: a record carries what the ones it adds hold",
    );
    assert!(!served.holds("albums/cover.png"), "and places no file");
}

// Browsing the Index needs no Storage, so a Storage that is away is not a server
// that refuses to start: what it holds is served, and the refresh is what asks
// again once there is a bucket to ask.
#[tokio::test]
async fn a_startup_that_cannot_reach_storage_still_serves_what_the_index_holds() {
    let served = Served::joined().await;
    served.halt_storage();

    served.start_up().await;
    assert!(
        served.refused_reads() > 0,
        "the startup did reach for the Library's head",
    );

    let (status, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(status, 200, "the server answers rather than being dead");
    assert_eq!(
        listed,
        json!({ "folders": [] }),
        "and answers with what this device knows, which is nothing yet",
    );

    served.resume_storage();
    let (status, refreshed) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    assert_eq!(refreshed["advanced"], true);

    let (_, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(
        listed,
        json!({ "folders": ["albums", "albums/2026", "books"] }),
        "the retry is the whole recovery: no restart, no terminal",
    );
}

// The other way Storage goes away, and the one that would otherwise never end:
// it takes the read and answers neither way, which is what a filtered network
// looks like from here. The catch-up runs before the socket is bound, so without
// a deadline of its own the explorer would not be unreachable for a while — it
// would be unreachable for as long as the silence lasted.
//
// The clock is the case's own: `start_paused` moves it forward whenever nothing
// is runnable, so the deadline is reached at once and nothing here waits a real
// minute. The outer timeout is a generous multiple of it, and is what fails this
// case rather than hanging it if the deadline is ever taken back out.
#[tokio::test(start_paused = true)]
async fn a_startup_that_storage_never_answers_gives_up_and_serves() {
    let served = Served::joined().await;
    served.stall_storage();

    let started = tokio::time::Instant::now();
    tokio::time::timeout(Duration::from_secs(600), served.start_up())
        .await
        .expect("the startup gives up on its own rather than waiting on Storage forever");
    assert!(
        served.stalled_reads() > 0,
        "the startup did reach for the Library's head",
    );
    assert!(
        started.elapsed() >= Duration::from_secs(30),
        "and waited on it, rather than never having gone out at all: {:?}",
        started.elapsed(),
    );

    let (status, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(status, 200, "the server answers rather than being dead");
    assert_eq!(
        listed,
        json!({ "folders": [] }),
        "and answers with what this device knows, which is nothing yet",
    );

    // What was abandoned leaves nothing behind that stops the next run: a
    // catch-up is replayed a record at a time, and the catalog's checkpoint is
    // where the one after it carries on from (spec: CK-9).
    served.resume_storage();
    let (status, refreshed) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    assert_eq!(refreshed["advanced"], true);

    let (_, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(
        listed,
        json!({ "folders": ["albums", "albums/2026", "books"] }),
        "the retry is the whole recovery here too: no restart, no terminal",
    );
}

// The ending a catch-up reaches no line by: the deadline drops the replay where
// it stands, so nothing in it runs to say how the catalog was left. A catalog
// still saying it is being caught up would be a browser told to wait for a run
// that no longer exists, and told it for as long as the tab is open.
#[tokio::test(start_paused = true)]
async fn a_catch_up_dropped_at_the_deadline_leaves_the_catalog_behind() {
    let served = Served::joined().await;
    served.stall_storage();

    tokio::time::timeout(Duration::from_secs(600), served.start_up())
        .await
        .expect("the startup gives up on its own");

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["catalog"]["state"], "behind");
    assert!(
        work["catalog"]["stopped"]["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "and says the one thing there is to say about a call that never came back: {work}",
    );
}

// The emptiness a joined device shows is not the emptiness of an empty Library,
// and the two are told apart here rather than left to a person to guess at. The
// listing is the same either way — it comes out of the catalog, and the catalog
// is what this device has replayed (spec: CK-9) — so what distinguishes them is
// the standing the server keeps of its own catch-up and answers beside the work
// it is doing.
#[tokio::test]
async fn an_empty_library_and_a_catalog_that_never_caught_up_are_told_apart() {
    let served = Served::joined().await;

    // Before the server has started up: nothing has been replayed, and the
    // route says so rather than letting an empty listing speak for it.
    let (_, before) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        before["catalog"],
        json!({ "state": "catching_up", "stopped": null })
    );

    served.halt_storage();
    served.start_up().await;

    let (_, behind) = body_of(served.get("/api/work").await).await;
    assert_eq!(behind["catalog"]["state"], "behind");
    assert!(
        behind["catalog"]["stopped"]["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "and says what stopped it, which is what turns waiting into trying again: {behind}",
    );
    let (_, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(
        listed,
        json!({ "folders": [] }),
        "the listing is as empty as it would be for a Library holding nothing",
    );

    served.resume_storage();
    served.start_up().await;

    let (_, caught_up) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        caught_up["catalog"],
        json!({ "state": "caught_up", "stopped": null }),
        "and a catch-up that landed takes the trouble off the screen with it",
    );
    let (_, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(
        listed,
        json!({ "folders": ["albums", "albums/2026", "books"] }),
        "which is the same route answering, from a catalog that now holds the Library",
    );
}

// The other half of the same distinction: a device that has caught up and lists
// nothing is a Library with nothing in it, and the answer says so plainly rather
// than leaving room to read it as a device that has not learnt what is there.
#[tokio::test]
async fn a_library_with_nothing_in_it_says_the_catalog_is_current() {
    let served = Served::mapping_only("albums").await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["catalog"]["state"], "caught_up");
}

// A refresh somebody pressed is the same catch-up, and it moves the same
// standing: one that lands says the catalog is current, and a Storage that is
// away puts it back to behind with what refused it.
#[tokio::test]
async fn a_refresh_that_storage_refuses_leaves_the_catalog_behind() {
    let served = Served::joined().await;
    served.halt_storage();

    let (status, _) = body_of(served.post("/api/refresh").await).await;
    assert_ne!(status, 200, "the refresh is refused");

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["catalog"]["state"], "behind");

    served.resume_storage();
    let (status, _) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        work["catalog"],
        json!({ "state": "caught_up", "stopped": null }),
    );
}

// What the refresh is for. Another device commits while this server is up, and
// pressing refresh is how the person looking at the folder finds out — the row
// appears, `remote`, and the bytes stay where they are until it is opened.
#[tokio::test]
async fn a_refresh_brings_in_what_another_device_committed() {
    let served = Served::library().await;
    assert_eq!(files(&listing_of(&served, "albums").await).len(), 3);

    served.commit_elsewhere("albums/late.jpg", b"late").await;
    assert_eq!(
        files(&listing_of(&served, "albums").await).len(),
        3,
        "nothing has told this device about it yet",
    );

    let (status, refreshed) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        (
            refreshed["advanced"].as_bool(),
            refreshed["gained"].as_i64(),
            refreshed["entries"].as_u64(),
        ),
        (Some(true), Some(1), Some(7)),
        "one Entry more than the six the fixture planted: {refreshed}",
    );

    let listing = listing_of(&served, "albums").await;
    assert_eq!(
        states(&listing),
        [
            ("caf\u{e9}.jpg".to_owned(), "remote".to_owned()),
            ("cover.png".to_owned(), "remote".to_owned()),
            ("late.jpg".to_owned(), "remote".to_owned()),
            ("notes.txt".to_owned(), "remote".to_owned()),
        ],
        "the new row is the Library's and not this device's: {listing}",
    );
    assert!(
        !served.holds("albums/late.jpg"),
        "a refresh brings over the catalog and not the bytes",
    );
}

// The ordinary answer, and the one a person presses refresh for most often: the
// catalog is where the Library is, and the screen says so rather than saying
// nothing.
#[tokio::test]
async fn a_refresh_with_nothing_new_says_the_catalog_is_up_to_date() {
    let served = Served::library().await;

    let (status, refreshed) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        (
            refreshed["advanced"].as_bool(),
            refreshed["gained"].as_i64(),
            refreshed["entries"].as_u64(),
        ),
        (Some(false), Some(0), Some(6)),
        "the head it stands at is the head there is: {refreshed}",
    );
}

// A Storage that has gone is what a refresh most often meets, and it is offered
// again rather than reported as the server failing: the catalog stands where it
// stood, every other route goes on answering out of it, and the next press is
// the whole of the recovery.
#[tokio::test]
async fn a_refresh_storage_cannot_answer_is_a_retryable_refusal() {
    let served = Served::library().await;
    served.commit_elsewhere("albums/late.jpg", b"late").await;
    served.halt_storage();

    let (status, refusal) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 502);
    assert_eq!(refusal["error"], "storage");
    assert!(
        refusal["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "the refusal says something a person could act on: {refusal}",
    );

    let (status, listing) = body_of(served.get("/api/list?path=albums").await).await;
    assert_eq!(status, 200, "the rest of the explorer is untouched");
    assert_eq!(files(&listing).len(), 3);

    served.resume_storage();
    let (status, refreshed) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    assert_eq!(refreshed["gained"], 1);
    assert_eq!(files(&listing_of(&served, "albums").await).len(), 4);
}

// Two presses at once — a double click, two tabs — replay one after the other
// rather than both at once, and both are answered. The second finds the head the
// first one reached, which is the ordinary "nothing new".
#[tokio::test]
async fn two_refreshes_at_once_replay_one_after_the_other() {
    let served = Served::library().await;
    served.commit_elsewhere("albums/late.jpg", b"late").await;

    let (first, second) = tokio::join!(served.post("/api/refresh"), served.post("/api/refresh"));
    let (first_status, first_body) = body_of(first).await;
    let (second_status, second_body) = body_of(second).await;
    assert_eq!(first_status, 200);
    assert_eq!(second_status, 200);

    let gained = [&first_body, &second_body]
        .map(|answered| answered["gained"].as_i64().expect("a count"))
        .iter()
        .sum::<i64>();
    assert_eq!(
        gained, 1,
        "the Entry is counted once: {first_body}, {second_body}",
    );
    assert_eq!(files(&listing_of(&served, "albums").await).len(), 4);
}
