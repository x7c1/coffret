//! The explorer's filter settings: what this device's explorer last had its
//! tree filtered by, read and kept as the device's own state beside the
//! appearance, kept across a server started afresh, and refused while the
//! Library is locked (spec: LA-43).

use std::time::Duration;

use serde_json::{json, Value};

use crate::support::{json as body_of, Served};

async fn keep(served: &Served, settings: Value) -> (u16, Value) {
    let (status, body) = body_of(served.post_json("/api/filter-settings", &settings).await).await;
    (status.as_u16(), body)
}

async fn read(served: &Served) -> Value {
    let (status, body) = body_of(served.get("/api/filter-settings").await).await;
    assert_eq!(status, 200, "{body}");
    body
}

async fn appearance(served: &Served) -> Value {
    let (status, body) = body_of(served.get("/api/appearance").await).await;
    assert_eq!(status, 200, "{body}");
    body
}

fn no_filter() -> Value {
    json!({ "name": "", "recentOn": false, "recent": 10 })
}

// LA-43: a device that never kept a filter is answered with none.
#[tokio::test]
async fn a_device_that_never_kept_a_filter_has_none() {
    let served = Served::library().await;
    assert_eq!(read(&served).await, no_filter());
}

// LA-43: what is kept is what the next page reads, and what a server started
// afresh over the same device reads too — the desktop app's next launch.
#[tokio::test]
async fn the_settings_kept_last_past_a_restart() {
    let mut served = Served::library().await;
    let settings = json!({ "name": "カバー", "recentOn": true, "recent": 3 });

    let (status, kept) = keep(&served, settings.clone()).await;
    assert_eq!(status, 200, "{kept}");
    assert_eq!(kept, settings);
    assert_eq!(read(&served).await, settings);

    served.restart();
    served.start_up().await;
    assert_eq!(read(&served).await, settings);

    let (status, _) = keep(&served, no_filter()).await;
    assert_eq!(status, 200);
    assert_eq!(read(&served).await, no_filter());
}

// LA-43: settings the explorer could not have been set to are refused, and
// what was kept stays.
#[tokio::test]
async fn settings_that_are_not_valid_are_refused() {
    let served = Served::library().await;
    let kept = json!({ "name": "albums", "recentOn": true, "recent": 5 });
    keep(&served, kept.clone()).await;

    let too_long = "a".repeat(coffret_device::MOST_FILTER_NAME_CHARS + 1);
    for asked in [
        json!({ "name": "x", "recentOn": true, "recent": 0 }),
        json!({ "name": "x", "recentOn": true, "recent": -2 }),
        json!({ "name": "x", "recentOn": true, "recent": 2.5 }),
        json!({ "name": "x", "recentOn": true, "recent": "3" }),
        json!({ "name": too_long, "recentOn": false, "recent": 10 }),
        json!({ "name": "x", "recentOn": false }),
        json!({ "name": "x", "recentOn": false, "recent": 10, "more": true }),
        json!(null),
    ] {
        let (status, refusal) = keep(&served, asked.clone()).await;
        assert_eq!(status, 400, "{asked}: {refusal}");
        assert_eq!(refusal["error"], "bad_request", "{asked}");
    }
    assert_eq!(read(&served).await, kept);
}

// LA-43: the filter settings and the appearance share one file, and keeping
// either leaves the other as it was.
#[tokio::test]
async fn keeping_one_leaves_the_other() {
    let served = Served::library().await;
    let (status, _) = body_of(
        served
            .post_json("/api/appearance", &json!({ "appearance": "dark" }))
            .await,
    )
    .await;
    assert_eq!(status, 200);

    let settings = json!({ "name": "books", "recentOn": false, "recent": 7 });
    keep(&served, settings.clone()).await;
    assert_eq!(appearance(&served).await, json!({ "appearance": "dark" }));

    let (status, _) = body_of(
        served
            .post_json("/api/appearance", &json!({ "appearance": "light" }))
            .await,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(read(&served).await, settings);
}

// LA-43: a file written before the filter settings were kept — the appearance
// alone — still reads, with no filter.
#[tokio::test]
async fn a_file_holding_the_appearance_alone_reads_with_no_filter() {
    let served = Served::library().await;
    served.write_explorer_file("{\n  \"appearance\": \"dark\"\n}\n");
    assert_eq!(read(&served).await, no_filter());
    assert_eq!(appearance(&served).await, json!({ "appearance": "dark" }));
}

// LA-43: a name typed to filter the tree says something about what the
// Library holds, so a locked server neither answers nor keeps it, and what
// was kept stays.
#[tokio::test]
async fn a_locked_server_neither_answers_nor_keeps_the_settings() {
    let mut served = Served::library().await;
    let kept = json!({ "name": "albums", "recentOn": false, "recent": 10 });
    keep(&served, kept.clone()).await;
    served.lock();

    let (status, refusal) = body_of(served.get("/api/filter-settings").await).await;
    assert_eq!(status, 423, "{refusal}");
    assert_eq!(refusal["error"], "locked");
    let (status, refusal) = keep(
        &served,
        json!({ "name": "books", "recentOn": false, "recent": 10 }),
    )
    .await;
    assert_eq!(status, 423, "{refusal}");
    assert_eq!(refusal["error"], "locked");

    served.restart();
    served.start_up().await;
    assert_eq!(read(&served).await, kept);
}

// LA-43, DK-4: neither route needs a key, so neither records somebody being
// here. Settings read and kept a third of the way into the idle interval do not
// put the lock off: it lands once the interval has passed since the start.
#[tokio::test(start_paused = true)]
async fn reading_and_keeping_the_settings_is_not_activity() {
    let quiet = Duration::from_secs(15 * 60);
    let served = Served::library().await;
    served.watch_idle(quiet).await;

    tokio::time::advance(quiet / 3).await;
    let (status, body) = keep(
        &served,
        json!({ "name": "albums", "recentOn": false, "recent": 10 }),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    read(&served).await;

    tokio::time::advance(quiet - quiet / 3 + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;

    let (status, refusal) = body_of(served.get("/api/filter-settings").await).await;
    assert_eq!(
        status, 423,
        "nobody wanted the Library, so it locked: {refusal}"
    );
    assert_eq!(refusal["error"], "locked");
}
