//! The explorer's appearance: which set of colours this device's explorer is
//! drawn in, read and kept as the device's own state, kept across a server
//! started afresh, and answered while the Library is locked (spec: LA-38).

use serde_json::{json, Value};

use crate::support::{json as body_of, Served};

async fn choose(served: &Served, appearance: Value) -> (u16, Value) {
    let (status, body) = body_of(
        served
            .post_json("/api/appearance", &json!({ "appearance": appearance }))
            .await,
    )
    .await;
    (status.as_u16(), body)
}

async fn read(served: &Served) -> Value {
    let (status, body) = body_of(served.get("/api/appearance").await).await;
    assert_eq!(status, 200, "{body}");
    body
}

// LA-38: a device that never chose follows the system.
#[tokio::test]
async fn a_device_that_never_chose_follows_the_system() {
    let served = Served::library().await;
    assert_eq!(read(&served).await, json!({ "appearance": "system" }));
}

// LA-38: what is chosen is what the next page reads, and what a server
// started afresh over the same device reads too — the desktop app's next
// launch, whose page has an origin of its own and none of the last one's
// browser storage.
#[tokio::test]
async fn a_choice_lasts_past_a_restart() {
    let mut served = Served::library().await;

    let (status, kept) = choose(&served, json!("dark")).await;
    assert_eq!(status, 200, "{kept}");
    assert_eq!(kept, json!({ "appearance": "dark" }));
    assert_eq!(read(&served).await, json!({ "appearance": "dark" }));

    served.restart();
    served.start_up().await;
    assert_eq!(read(&served).await, json!({ "appearance": "dark" }));

    let (status, _) = choose(&served, json!("system")).await;
    assert_eq!(status, 200);
    assert_eq!(read(&served).await, json!({ "appearance": "system" }));
}

// LA-38: a set this server does not draw in is refused, and what was kept
// stays.
#[tokio::test]
async fn a_set_that_is_not_one_of_the_three_is_refused() {
    let served = Served::library().await;
    choose(&served, json!("light")).await;

    for asked in [json!("sepia"), json!(null), json!(1)] {
        let (status, refusal) = choose(&served, asked.clone()).await;
        assert_eq!(status, 400, "{asked}: {refusal}");
        assert_eq!(refusal["error"], "bad_request", "{asked}");
    }
    assert_eq!(read(&served).await, json!({ "appearance": "light" }));
}

// LA-38: the appearance is the device's and needs no key, so a locked server
// still reads and keeps it (the explorer host's fence is what refuses a page
// asking while the Library is locked).
#[tokio::test]
async fn a_locked_server_still_reads_and_keeps_the_appearance() {
    let served = Served::library().await;
    served.lock();

    let (status, kept) = choose(&served, json!("light")).await;
    assert_eq!(status, 200, "{kept}");
    assert_eq!(read(&served).await, json!({ "appearance": "light" }));
}
