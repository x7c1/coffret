//! Locked and unlocked.
//!
//! A command line process is one unlock and one run, so it never has these two
//! states to be in. A server does: the Passphrase was spent at startup, and what
//! it produced lives until a lock ends it (spec: DK-1). These are the cases over
//! the lock the clock makes, and what a locked server does — to the routes
//! that need a key, and to work that is already running.
//!
//! Most of the cases about the locked state reach it through the state the
//! server holds rather than through the clock: what they are about is being
//! locked, and the clock is the business of the cases about the interval.

use std::time::Duration;

use crate::support::{bytes, json as body_of, route, Served};

/// Every route that cannot answer without the Master Key, as a caller reaches
/// one.
///
/// The upload is not here and is asked separately: what it takes is a multipart
/// body, and a request without one is refused by the multipart extractor rather
/// than by the lock — so the case that means to be about the lock sends a real
/// drop.
const KEYED_ROUTES: [(&str, &str); 7] = [
    ("GET", "/api/folders"),
    ("GET", "/api/list?path=albums"),
    ("GET", "/api/file?path=albums/cover.png"),
    ("POST", "/api/fill?path=albums"),
    ("POST", "/api/sync"),
    ("POST", "/api/freeze?path=albums"),
    ("POST", "/api/refresh"),
];

/// The idle interval the cases about the clock run under.
///
/// A quarter of an hour these cases name for themselves, because the idle
/// interval is a policy parameter rather than a format constant (spec: DK-4)
/// and the parameter is what they are about: a case that ran under the interval
/// the server ships with would be testing that one value rather than the rule it
/// is a value of. How long a device stays unlocked past that interval is the
/// user's own choice (spec: DK-9), which no case here fixes.
const QUIET: Duration = Duration::from_secs(15 * 60);

// DK-2 over every route at once: once the server is locked, the very next
// request that needs a key finds nothing left to work with. The routes that
// never needed a key go on answering, which is what keeps a locked server
// something a person can still read the name of rather than a process that has
// gone silent.
#[tokio::test]
async fn a_lock_shuts_every_route_that_needs_a_key() {
    let served = Served::library().await;
    let (status, _) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 200, "it is open to begin with");

    served.lock();

    for (method, uri) in KEYED_ROUTES {
        let (status, refusal) = route(&served, method, uri).await;
        assert_eq!(status, 423, "{method} {uri} answered a locked server");
        assert_eq!(refusal["error"], "locked", "{method} {uri}");
    }

    let (status, library) = body_of(served.get("/api/library").await).await;
    assert_eq!(
        status, 200,
        "which Library this is is not a thing the Master Key keeps",
    );
    assert_eq!(library["name"], "served");
    let (status, _) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        status, 200,
        "and neither is this server's own account of what it was doing",
    );
}

// DK-2: none of them partially succeeds. A drop onto a locked server is refused
// whole — before a byte of any part reaches the folder — rather than landing
// files that no flow could ever carry in.
#[tokio::test]
async fn a_drop_onto_a_locked_server_lands_nothing() {
    let served = Served::library().await;
    served.lock();

    let (status, refusal) = body_of(
        served
            .upload("albums", &[("first.txt", b"one"), ("second.txt", b"two")])
            .await,
    )
    .await;
    assert_eq!(status, 423);
    assert_eq!(refusal["error"], "locked");
    assert!(!served.holds("albums/first.txt"), "and wrote nothing");
    assert!(!served.holds("albums/second.txt"));
}

// The sentence DK-2 asks for, said in the words the register uses, and said to
// somebody who can act on it: the Passphrase is what opens this, and starting
// the server again is where a Passphrase is typed. It is a kind of its own and
// not the admission fence's `unauthorized` — being locked is the owner's own
// state, not somebody else being turned away, so the answer tells them
// everything rather than nothing.
#[tokio::test]
async fn a_locked_server_says_the_passphrase_is_required() {
    let served = Served::library().await;
    served.lock();

    let (status, refusal) = body_of(served.get("/api/file?path=albums/cover.png").await).await;
    assert_eq!(status, 423);
    assert_eq!(refusal["error"], "locked");
    let said = refusal["message"]
        .as_str()
        .expect("a refusal carries one sentence")
        .to_owned();
    assert!(
        said.contains("Passphrase"),
        "it names what is needed: {said}"
    );
    assert!(
        said.contains("starting it again"),
        "and how to provide it: {said}",
    );
}

// DK-2 said about a server that answers many callers at once: what the lock
// ends is the next piece of work, not the one already running. The idle lock
// reads the clock and then empties the cell, and a request can take its handle
// on the keys in between; a fetch that did finishes with it and answers with
// the Entry — which is what "none of them partially succeeds" means per
// operation rather than per connection.
//
// The request is provably in flight rather than probably: Storage takes the read
// and holds it until this case lets go, so the lock lands while the fetch is
// inside the server and nowhere else.
#[tokio::test]
async fn a_request_in_flight_when_the_lock_lands_finishes() {
    let served = Served::library().await;
    served.hold_storage();

    let (answer, ()) = tokio::join!(served.get("/api/file?path=albums/2026/spring.jpg"), async {
        // No sleep and no guess: the read is counted as it arrives, so this
        // waits for the fetch to be inside Storage.
        while served.held_reads() == 0 {
            tokio::task::yield_now().await;
        }
        served.lock();
        served.release_storage();
    },);

    assert_eq!(
        answer.status(),
        200,
        "the fetch that began unlocked finishes"
    );
    assert_eq!(bytes(answer).await, b"spring");
    assert!(
        served.holds("albums/2026/spring.jpg"),
        "and placed the file whole (spec: EP-11)",
    );

    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 423, "what asks after the lock is refused");
    assert_eq!(refusal["error"], "locked");
}

// The work nobody asked for meets the same lock, and stops rather than half
// running: nothing is placed, and what the browser polls says why in the same
// words a refused request would have used. A fill that pressed on would be one
// Storage call per file to be told the same thing once per file.
#[tokio::test]
async fn background_work_that_meets_a_lock_stops_cleanly() {
    let served = Served::library().await;
    served.lock();

    served.arm_fill("albums");
    served.fill_idle().await;

    let (status, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(status, 200);
    let fill = &work["fill"];
    assert_eq!(fill["status"], "stopped");
    assert_eq!(fill["done"], 0);
    assert_eq!(fill["stopped"]["error"], "locked");
    let said = fill["stopped"]["message"]
        .as_str()
        .expect("a stopped run carries one sentence");
    assert!(said.contains("Passphrase"), "{said}");
    assert!(
        !served.holds("albums/cover.png"),
        "and brought nothing over",
    );
}

// DK-4: inactivity for the configured interval locks the device. The clock is
// the case's own — `start_paused` is what lets a quarter of an hour be stated
// rather than spent — and nothing is asked of the server while it passes.
#[tokio::test(start_paused = true)]
async fn nothing_asked_for_the_idle_interval_locks_the_library() {
    let served = Served::library().await;
    served.watch_idle(QUIET).await;

    let (status, _) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 200, "it is open while somebody is here");

    tokio::time::advance(QUIET + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;

    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(
        status, 423,
        "and shut once nobody has been for the interval"
    );
    assert_eq!(refusal["error"], "locked");
    let (status, _) = body_of(served.get("/api/library").await).await;
    assert_eq!(status, 200, "the identity route answers either way");
}

// DK-4 from the other direction, and the case that makes the interval mean
// "quiet since somebody last wanted the Library" rather than "up for this
// long": three quarters of an hour pass in steps none of which is a quarter of
// an hour of silence, and the Library is still open at the end of them. What is
// asked is a route that needs the keys, because that is what being here means.
#[tokio::test(start_paused = true)]
async fn steady_activity_keeps_the_library_unlocked() {
    let served = Served::library().await;
    served.watch_idle(QUIET).await;

    for step in 1..=6 {
        tokio::time::advance(QUIET / 2).await;
        let (status, _) = route(&served, "GET", "/api/folders").await;
        assert_eq!(status, 200, "step {step}");
    }

    let (status, _) = route(&served, "GET", "/api/folders").await;
    assert_eq!(
        status, 200,
        "no quarter of an hour of it was quiet, so nothing locked it",
    );
}

// And the mirror of it, which is the case the idle lock exists for: a request
// that needs no key is not activity (spec: DK-4). A tab left open on a page
// asks this server what it is doing several times a second, and none of that
// is a person at the keyboard. The same three quarters of an hour pass in the
// same steps — every one of them a request this server answers — and the
// Library locks anyway.
#[tokio::test(start_paused = true)]
async fn steady_polling_for_activity_does_not_keep_the_library_unlocked() {
    let served = Served::library().await;
    served.watch_idle(QUIET).await;

    for step in 1..=6 {
        tokio::time::advance(QUIET / 2).await;
        let answer = served.get("/api/work").await;
        assert_eq!(
            answer.status(),
            200,
            "step {step}: the polling is answered either way",
        );
    }

    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(
        status, 423,
        "nobody wanted the Library for a quarter of an hour, so it locked",
    );
    assert_eq!(refusal["error"], "locked");
}

// DK-4 told to the one who cannot otherwise hear it. A device that locked itself
// did so on its own clock and asked nobody, and a window left open over a page
// it decrypted has no next request to be refused until somebody turns the page —
// so it goes on showing plaintext the key behind is gone from. The answer about
// what this server is doing carries the state, and the window reads the lock out
// of it.
//
// And it carries it without becoming activity: the interval runs out here under
// the very polling that reports it, which is the same rule
// `steady_polling_for_activity_does_not_keep_the_library_unlocked` states from
// the other side.
#[tokio::test(start_paused = true)]
async fn a_device_that_locked_itself_says_so_when_asked_what_it_is_doing() {
    let served = Served::library().await;
    served.watch_idle(QUIET).await;

    let (status, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        work["library"], "unlocked",
        "the Library is open while somebody is here",
    );

    tokio::time::advance(QUIET + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;

    let (status, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        status, 200,
        "the route answers a locked server exactly as it answered an open one",
    );
    assert_eq!(
        work["library"], "locked",
        "and says the quiet ended the Library's being open",
    );
    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 423, "which is the state the keyed routes are in");
    assert_eq!(refusal["error"], "locked");
}

// DK-4, and the span rather than the moment: the interval is quiet since
// somebody last wanted the Library, and wanting it lasts as long as the work
// does. A book being packed can take longer than a quarter of an hour by
// itself, and a lock landing in the middle of that would leave the very work it
// interrupted with nowhere to go: the run finishes on the handle it holds, and
// everything it arms next is refused — so the explorer offers to pack again and
// cannot. Storage holds the read here for what a long piece of work is, and the
// interval is counted afresh from the end of it rather than from its first
// moment.
#[tokio::test(start_paused = true)]
async fn work_that_outlasts_the_interval_defers_the_lock() {
    let served = Served::library().await;
    served.watch_idle(QUIET).await;
    served.hold_storage();

    let (answer, ()) = tokio::join!(served.get("/api/file?path=albums/2026/spring.jpg"), async {
        // No sleep and no guess: the read is counted as it arrives, so the
        // clock is only moved once the handle on the keys is out.
        while served.held_reads() == 0 {
            tokio::task::yield_now().await;
        }
        tokio::time::advance(QUIET * 2).await;
        served.release_storage();
    },);
    assert_eq!(answer.status(), 200, "the long piece of work finishes");
    // A fetch arms a fill of the folder around it, and that run holds a handle
    // of its own: it is work over the Library in exactly the sense the request
    // was, so the quiet begins once it is done too.
    served.fill_idle().await;

    tokio::time::advance(QUIET / 2).await;
    tokio::task::yield_now().await;
    let (status, _) = route(&served, "GET", "/api/folders").await;
    assert_eq!(
        status, 200,
        "and what comes after it is served, the interval starting at its end",
    );

    tokio::time::advance(QUIET + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 423, "deferred, and not called off");
    assert_eq!(refusal["error"], "locked");
}

// Where the first interval begins. Opening the Library and catching its catalog
// up with what the Library has become both happen before the socket answers
// anything, and a Storage that answers slowly can make the second of them a
// wait of its own — none of it time anybody could have been at the keyboard for.
// The watcher marks the start as it begins to watch, so what the first interval
// measures is the first quiet a person could have kept: a server whose startup
// ran longer than the interval serves rather than arriving already locked.
#[tokio::test(start_paused = true)]
async fn the_first_interval_is_counted_from_where_the_serving_starts() {
    let served = Served::library().await;
    // What starting up cost, stated rather than spent, and all of it before
    // there is anything watching.
    tokio::time::advance(QUIET * 2).await;
    served.watch_idle(QUIET).await;

    tokio::time::advance(QUIET / 2).await;
    tokio::task::yield_now().await;
    let (status, _) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 200, "none of the startup was the interval");

    tokio::time::advance(QUIET + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 423, "and the quiet after somebody was here is");
    assert_eq!(refusal["error"], "locked");
}

// An interval nobody could reach the end of. The parser refuses nothing above
// its minimum, and the minutes are turned into seconds saturatingly, so a number
// large enough is a wait no clock can add up — which must leave the watcher
// waiting rather than take it out with a panic on the sum. A watcher that had
// panicked is a finished task, and a Library nothing will ever lock.
#[tokio::test(start_paused = true)]
async fn an_interval_longer_than_the_clock_neither_panics_nor_locks() {
    let served = Served::library().await;
    let watcher = served.watch_idle(Duration::from_secs(u64::MAX)).await;

    tokio::time::advance(Duration::from_secs(365 * 24 * 60 * 60)).await;
    tokio::task::yield_now().await;

    assert!(
        !watcher.is_finished(),
        "the watcher is still waiting rather than gone",
    );
    let (status, _) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 200, "and a year of quiet was not the interval");
}
