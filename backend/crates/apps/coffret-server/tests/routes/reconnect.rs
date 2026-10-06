//! Renewing a grant Storage stopped taking, from the explorer, without
//! restarting the server.
//!
//! The consent flow itself — the page, the browser, Google's token endpoint —
//! is stood in for (`ScriptedConsent`), and ended by the case. What is real is
//! everything around it: the refusal the grant running out reaches a browser
//! as, the route that starts the flow, the one flow two presses share, the
//! server going on with the renewed grant, and the catch-up after it.

use std::time::Duration;

use coffret_device::Unrenewed;
use serde_json::{json, Value};

use crate::support::{json as body_of, route, Served, CONSENT_PAGE};

/// What the server is doing, as the explorer asks it.
async fn work(served: &Served) -> Value {
    let (status, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(status, 200);
    work
}

/// The work answer once its reconnect has left `waiting`.
///
/// The flow is followed on a task of the server's own, so its ending lands a
/// moment after the case ends it. Bounded, so a server that never records the
/// ending fails this case rather than hanging it.
async fn once_ended(served: &Served) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let work = work(served).await;
            if work["reconnect"]["state"] != "waiting" {
                return work;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the reconnect ends once its flow does")
}

// The whole of it, in the order a person meets it. A grant that ran out leaves
// the startup catch-up refused with the reason the explorer offers a reconnect
// from; the reconnect answers with the consent page; a second press while it
// waits is the same page and no second flow; and once the person has consented,
// the server reaches Storage with the renewed grant — no restart — and catches
// the catalog up, which is what puts the Library back on the screen.
#[tokio::test]
async fn a_grant_that_ran_out_is_renewed_and_the_catalog_caught_up_without_a_restart() {
    let served = Served::joined().await;
    served.halt_storage();
    served.start_up().await;

    let behind = work(&served).await;
    assert_eq!(behind["catalog"]["state"], "behind", "{behind}");
    assert_eq!(behind["catalog"]["stopped"]["error"], "storage");
    assert_eq!(
        behind["catalog"]["stopped"]["reason"], "unauthenticated",
        "the catch-up the grant refused says so, so the page can offer a reconnect: {behind}",
    );
    assert_eq!(behind["reconnect"], Value::Null, "nothing reconnected yet");

    let (status, started) = body_of(served.post("/api/reconnect").await).await;
    assert_eq!(status, 202, "{started}");
    assert_eq!(started["url"], CONSENT_PAGE);
    assert!(started["message"].is_string(), "{started}");
    assert_eq!(work(&served).await["reconnect"]["state"], "waiting");

    let (status, again) = body_of(served.post("/api/reconnect").await).await;
    assert_eq!(status, 202);
    assert_eq!(
        again["url"], CONSENT_PAGE,
        "a second press while the flow waits answers with the same page",
    );
    assert_eq!(served.consent.asked(), 1, "and starts no second flow");
    assert!(
        work(&served).await["reconnect"].get("url").is_none(),
        "the page goes back to whoever pressed, and to nobody polling",
    );

    // The person consents: the grant is cached, and Storage takes it.
    served.resume_storage();
    served.consent.end(Ok(()));

    let renewed = once_ended(&served).await;
    assert_eq!(renewed["reconnect"]["state"], "renewed", "{renewed}");
    assert_eq!(
        renewed["catalog"],
        json!({ "state": "caught_up", "stopped": null }),
        "the catalog was caught up with the renewed grant",
    );
    let (_, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(
        listed,
        json!({ "folders": ["albums", "albums/2026", "books"] }),
        "refused before, served after, by the same process",
    );
}

// The two endings a person brings about without a grant, each told as itself so
// the page can offer the button again with the reason beside it; and nothing
// is caught up after either, since the grant is the one that ran out.
#[tokio::test]
async fn a_declined_or_unanswered_consent_is_reported_and_can_be_asked_again() {
    let served = Served::joined().await;
    served.halt_storage();
    served.start_up().await;

    for (ending, said) in [
        (Unrenewed::Refused, "refused"),
        (Unrenewed::TimedOut, "timed_out"),
        (Unrenewed::Failed, "failed"),
    ] {
        let (status, _) = body_of(served.post("/api/reconnect").await).await;
        assert_eq!(status, 202);
        served.consent.end(Err(ending));

        let ended = once_ended(&served).await;
        assert_eq!(ended["reconnect"]["state"], said, "{ended}");
        assert_eq!(ended["catalog"]["state"], "behind", "{ended}");
    }
    assert_eq!(
        served.consent.asked(),
        3,
        "a flow that ended is not one a later press is answered from",
    );
}

// A reconnect needs the Library's keys like every other piece of work, so a
// locked server refuses it the way it refuses the rest (spec: DK-2) — and
// starts no flow.
#[tokio::test]
async fn a_locked_server_refuses_a_reconnect() {
    let served = Served::library().await;
    served.lock();

    let (status, refusal) = body_of(served.post("/api/reconnect").await).await;
    assert_eq!(status, 423, "{refusal}");
    assert_eq!(refusal["error"], "locked");
    assert_eq!(served.consent.asked(), 0);
}

// A flow waiting on its consent page is work over the Library's keys, so the
// idle interval does not run out under it (spec: DK-4) — a person who switched
// to the consent tab is somebody being here. And it holds them only while it
// waits: once the flow ends, however it ends, the handle goes with it and the
// interval is counted from there, so a flow nobody answers cannot keep the
// Library unlocked past its own timeout.
#[tokio::test(start_paused = true)]
async fn a_waiting_flow_defers_the_idle_lock_and_lets_go_when_it_ends() {
    let quiet = Duration::from_secs(15 * 60);
    let served = Served::library().await;
    served.watch_idle(quiet).await;

    let (status, _) = body_of(served.post("/api/reconnect").await).await;
    assert_eq!(status, 202);

    tokio::time::advance(quiet * 2).await;
    tokio::task::yield_now().await;
    assert_eq!(
        work(&served).await["reconnect"]["state"],
        "waiting",
        "the flow is still waiting on the person",
    );
    let (status, _) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 200, "and the Library is open while it does");

    served.consent.end(Err(Unrenewed::TimedOut));
    assert_eq!(once_ended(&served).await["reconnect"]["state"], "timed_out");

    tokio::time::advance(quiet + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 423, "a flow that ended holds nothing: {refusal}");
    assert_eq!(refusal["error"], "locked");
}
