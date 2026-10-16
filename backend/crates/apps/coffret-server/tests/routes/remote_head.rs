//! Looking at which heads the Library has, by name, without catching up
//! (spec: CK-9, FM-12, DK-4).

use std::time::Duration;

use serde_json::json;

use crate::support::{files, json as body_of, listing_of, route, Asked, Served};

/// The idle interval the case about the clock runs under, as `lock.rs` names
/// its own: a quarter of an hour of the policy parameter (spec: DK-4).
const QUIET: Duration = Duration::from_secs(15 * 60);

/// Asserts that every Storage call since reads were last forgotten was a page
/// of the listing, and that there was at least one.
fn listed_only(served: &Served) {
    let asked = served.asked();
    assert!(!asked.is_empty(), "the look did reach Storage");
    assert!(
        asked.iter().all(|asked| *asked == Asked::Listed),
        "names only: no object was read, so no key was reached for: {asked:?}",
    );
}

// The whole of what the route is for. Another device commits, and the look says
// the head is newer than the catalog's without reading anything but names; a
// catch-up brings the catalog to the head, and the look says it is current.
#[tokio::test]
async fn the_remote_head_is_newer_after_another_device_commits_and_current_after_a_catch_up() {
    let served = Served::library().await;

    let (status, looked) = body_of(served.get("/api/remote-head").await).await;
    assert_eq!(status, 200);
    assert_eq!(looked, json!({ "head": "current" }));
    listed_only(&served);

    served.commit_elsewhere("albums/late.jpg", b"late").await;
    served.forget_reads();
    let (status, looked) = body_of(served.get("/api/remote-head").await).await;
    assert_eq!(status, 200);
    assert_eq!(looked, json!({ "head": "newer" }));
    listed_only(&served);

    let (status, _) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    served.forget_reads();
    let (status, looked) = body_of(served.get("/api/remote-head").await).await;
    assert_eq!(status, 200);
    assert_eq!(looked, json!({ "head": "current" }));
    listed_only(&served);
}

// A look moves nothing. The catalog stands where it stood — the rows the folder
// lists are the ones it listed, and the server's own account of its catch-up is
// unchanged — however many times somebody looks.
#[tokio::test]
async fn looking_at_the_remote_head_leaves_the_catalog_where_it_stands() {
    let served = Served::library().await;
    served.commit_elsewhere("albums/late.jpg", b"late").await;
    let (_, before) = body_of(served.get("/api/work").await).await;

    for _ in 0..2 {
        let (status, looked) = body_of(served.get("/api/remote-head").await).await;
        assert_eq!(status, 200);
        assert_eq!(looked, json!({ "head": "newer" }));
    }

    assert_eq!(
        files(&listing_of(&served, "albums").await).len(),
        3,
        "the Entry the other device committed is not in this catalog yet",
    );
    let (_, after) = body_of(served.get("/api/work").await).await;
    assert_eq!(after["catalog"], before["catalog"], "{after}");
}

// A look does not wait for the turn a catch-up holds: it replays nothing, so
// there is nothing two callers could do twice. A refresh is stopped inside
// Storage, holding the turn, and the look is answered all the same — before
// the refresh is let go.
#[tokio::test]
async fn looking_at_the_remote_head_does_not_wait_for_a_catch_up() {
    let served = Served::library().await;
    served.commit_elsewhere("albums/late.jpg", b"late").await;
    served.hold_next_whole_read();

    let refreshing = served.post("/api/refresh");
    let looking = async {
        while served.held_reads() == 0 {
            tokio::task::yield_now().await;
        }
        let looked = body_of(served.get("/api/remote-head").await).await;
        served.release_storage();
        looked
    };
    let (refreshed, (status, looked)) = tokio::join!(refreshing, looking);

    assert_eq!(status, 200);
    assert_eq!(
        looked,
        json!({ "head": "newer" }),
        "answered while the catch-up held the turn, before it reached the head",
    );
    let (status, refreshed) = body_of(refreshed).await;
    assert_eq!(status, 200);
    assert_eq!(refreshed["advanced"], true, "{refreshed}");
}

// A tab left open looks on a timer, and that is not a person wanting the
// Library (spec: DK-4): the same three quarters of an hour of looks the
// activity cases in `lock.rs` take, and the Library locks anyway.
#[tokio::test(start_paused = true)]
async fn looking_at_the_remote_head_does_not_keep_the_library_unlocked() {
    let served = Served::library().await;
    served.watch_idle(QUIET).await;

    tokio::time::advance(QUIET / 2).await;
    let (status, _) = route(&served, "GET", "/api/remote-head").await;
    assert_eq!(status, 200, "the first look is answered");
    for _ in 2..=6 {
        tokio::time::advance(QUIET / 2).await;
        route(&served, "GET", "/api/remote-head").await;
    }

    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(
        status, 423,
        "nobody wanted the Library for a quarter of an hour, so it locked",
    );
    assert_eq!(refusal["error"], "locked");
}

// Storage being away is a refusal like a refresh's, so the explorer keeps what
// it last drew and asks again later; the catalog's own account is untouched by
// it, since nothing was being caught up.
#[tokio::test]
async fn a_look_storage_refuses_is_a_storage_refusal_and_leaves_the_catalog_alone() {
    let served = Served::library().await;
    served.refuse_listing();

    let (status, refusal) = body_of(served.get("/api/remote-head").await).await;
    assert_eq!(status, 502, "{refusal}");
    assert_eq!(refusal["error"], "storage");

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["catalog"]["state"], "caught_up", "{work}");
}
