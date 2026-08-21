//! The gateway's classification steering the policy's loop.
//!
//! Two things are already proved separately.
//! [`classification_tests`](crate::classification_tests) proves that each of
//! Drive's answers becomes the right port error, and the policy's own cases
//! prove that the loop behaves correctly when it is handed errors. Neither
//! proves the wire between them: that the verdict this gateway reaches is the
//! verdict the loop acts on.
//!
//! That junction is where a mistake would be most expensive and least visible.
//! A limit classified as throttling would have a worker spend its whole waiting
//! budget on a Drive that is full, and every log line along the way would look
//! like an ordinary bad minute. So these cases drive the real gateway through a
//! real [`RetryPolicy`], against the scripted transport — and assert what went
//! over the wire, because the number of requests is the only place the
//! difference shows.
//!
//! Every case runs under a paused clock, so a policy that waits seconds waits
//! none of them in the suite.

use std::time::Duration;

use coffret_logging::testing::CapturedLogs;
use coffret_usecase::{Error, ObjectStore, RetryPolicy};
use tokio::time::Instant;
use tracing::Level;

use crate::http::StubAnswer;
use crate::test_support::scripted_drive;

/// Drive's error envelope for one reason.
fn envelope(reason: &str) -> String {
    format!(r#"{{"error":{{"message":"{reason}","errors":[{{"reason":"{reason}"}}]}}}}"#)
}

/// What Drive answers a listing of a folder holding one object with.
const ONE_OBJECT: &str = r#"{"files":[{"id":"file-1","name":"jrn-1.cfrt","size":"4096"}]}"#;

/// A policy small enough to read, and generous enough to honour a `Retry-After`.
///
/// Three attempts, so the answers a case has to script are the answers it can
/// hold in view. The base backoff is in milliseconds and the ceiling in tens of
/// seconds, which is what makes a wait of three seconds below unmistakably the
/// figure Drive named rather than a computed one.
fn brisk() -> RetryPolicy {
    RetryPolicy::default()
        .with_attempts(3)
        .with_base_backoff(Duration::from_millis(100))
        .with_wait_ceiling(Duration::from_secs(30))
        .with_total_wait(Duration::from_secs(60))
}

#[tokio::test(start_paused = true)]
async fn throttling_that_passes_is_ridden_out_rather_than_reported() {
    let (store, transport, _) = scripted_drive([
        StubAnswer::json(429, &envelope("rateLimitExceeded")),
        StubAnswer::json(429, &envelope("rateLimitExceeded")),
        StubAnswer::json(200, ONE_OBJECT),
    ]);

    let page = brisk()
        .run("list", || store.list(None))
        .await
        .expect("the third answer succeeds, so the call does");

    assert_eq!(page.objects.len(), 1);
    assert_eq!(
        transport.call_count(),
        3,
        "two refusals worth waiting out are two more calls, and no more",
    );
}

#[tokio::test(start_paused = true)]
async fn throttling_dressed_as_a_refusal_is_ridden_out_too() {
    // The shape Drive actually throttles with, and a different path into
    // `RateLimited` than the 429 above: a status this gateway otherwise reads
    // as access refused, told apart by its reason alone.
    let (store, transport, _) = scripted_drive([
        StubAnswer::json(403, &envelope("userRateLimitExceeded")),
        StubAnswer::json(403, &envelope("userRateLimitExceeded")),
        StubAnswer::json(200, ONE_OBJECT),
    ]);

    let page = brisk()
        .run("list", || store.list(None))
        .await
        .expect("the third answer succeeds, so the call does");

    assert_eq!(page.objects.len(), 1);
    assert_eq!(transport.call_count(), 3);
}

#[tokio::test(start_paused = true)]
async fn a_wait_drive_asks_for_by_name_is_the_wait_taken() {
    let asked = Duration::from_secs(3);
    let (store, transport, _) = scripted_drive([
        StubAnswer::json_with_headers(
            429,
            vec![("retry-after".to_owned(), "3".to_owned())],
            &envelope("rateLimitExceeded"),
        ),
        StubAnswer::json(200, ONE_OBJECT),
    ]);

    let started = Instant::now();
    brisk()
        .run("list", || store.list(None))
        .await
        .expect("the second answer succeeds, so the call does");

    // A lower bound and no upper one: the computed wait this replaced is drawn
    // from zero to 100ms, so three seconds could not have come from anywhere
    // but the header. Pinning the figure exactly would be pinning the tick the
    // timer happened to serve the sleeper on.
    assert!(
        started.elapsed() >= asked,
        "Drive asked to be left alone for {asked:?} and was left for {:?}",
        started.elapsed(),
    );
    assert_eq!(transport.call_count(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_full_drive_is_answered_at_once_instead_of_spending_the_budget() {
    // Scripted to answer three times over, so that a wrong classification here
    // shows up as a request count rather than as the transport running out of
    // script. That count is the whole case: `LimitReached` and `RateLimited`
    // are both refusals a caller reports, and the only visible difference
    // between them is whether a worker asked a full Drive again.
    let (store, transport, _) = scripted_drive([
        StubAnswer::json(403, &envelope("storageQuotaExceeded")),
        StubAnswer::json(403, &envelope("storageQuotaExceeded")),
        StubAnswer::json(403, &envelope("storageQuotaExceeded")),
    ]);

    let started = Instant::now();
    let error = brisk()
        .run("list", || store.list(None))
        .await
        .expect_err("a Drive that is full does not empty by being asked again");

    let Error::LimitReached { limit, .. } = &error else {
        panic!("a full Drive is a limit reached, not access refused: {error:?}");
    };
    assert_eq!(limit, "storageQuotaExceeded");
    assert_eq!(
        transport.call_count(),
        1,
        "a limit stays reached however many times it is asked about",
    );
    assert_eq!(
        started.elapsed(),
        Duration::ZERO,
        "and however long anyone waits",
    );
}

#[tokio::test(start_paused = true)]
async fn throttling_that_never_lets_up_gives_up_at_a_bound_and_records_which() {
    let logs = CapturedLogs::capture();
    let (store, transport, _) = scripted_drive([
        StubAnswer::json(429, &envelope("rateLimitExceeded")),
        StubAnswer::json(429, &envelope("rateLimitExceeded")),
        StubAnswer::json(429, &envelope("rateLimitExceeded")),
    ]);

    let error = brisk()
        .run("list", || store.list(None))
        .await
        .expect_err("throttling that outlasts the attempts fails the call");

    // What Drive said last, rather than a synthetic "ran out of attempts".
    assert!(matches!(error, Error::RateLimited { .. }), "{error:?}");
    assert_eq!(transport.call_count(), 3, "three attempts, and no fourth");

    let event = logs.only(Level::WARN);
    assert!(event.message().contains("gave up"), "{event}");
    assert_eq!(event.field("operation"), "list");
    assert_eq!(
        event.field("bound"),
        "attempts",
        "a provider having a bad minute, rather than one that kept asking to be left alone: {event}",
    );
    assert_eq!(event.number("attempts"), 3);
}
