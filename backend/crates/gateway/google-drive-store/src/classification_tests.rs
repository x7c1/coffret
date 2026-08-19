//! Which failures are worth another attempt.
//!
//! A storage layer that cannot tell throttling from refusal either gives up on
//! a Library it is entitled to, or hammers an endpoint that will never say yes.
//! Drive reports both as a 403, and both as prose in a message — so the
//! classification happens once, here, and everything above works from the type.
//!
//! None of these can be provoked against the real API on demand, which is the
//! whole reason the transport is injected.

use coffret_usecase::{Error, ObjectStore};

use crate::http::{StubAnswer, TransportError};
use crate::test_support::scripted_drive;

/// Drive's error envelope for one reason.
fn envelope(reason: &str) -> String {
    format!(r#"{{"error":{{"message":"{reason}","errors":[{{"reason":"{reason}"}}]}}}}"#)
}

/// The error a listing comes back with when Drive answers this.
async fn listing_error(answer: StubAnswer) -> Error {
    let (store, _, _) = scripted_drive([answer]);
    store
        .list(None)
        .await
        .expect_err("the scripted answer must fail the call")
}

#[tokio::test]
async fn too_many_requests_is_worth_waiting_out() {
    let error = listing_error(StubAnswer::json(429, &envelope("rateLimitExceeded"))).await;

    assert!(matches!(error, Error::RateLimited { .. }), "{error:?}");
    assert!(error.is_retryable());
}

#[tokio::test]
async fn throttling_dressed_as_a_refusal_is_worth_waiting_out() {
    let error = listing_error(StubAnswer::json(403, &envelope("userRateLimitExceeded"))).await;

    assert!(matches!(error, Error::RateLimited { .. }), "{error:?}");
    assert!(error.is_retryable());
}

#[tokio::test]
async fn a_fault_on_drives_side_is_worth_another_attempt() {
    let error = listing_error(StubAnswer::json(503, &envelope("backendError"))).await;

    assert!(
        matches!(error, Error::ServiceUnavailable { status: 503, .. }),
        "{error:?}"
    );
    assert!(error.is_retryable());
}

#[tokio::test]
async fn a_call_that_ran_out_of_time_is_worth_another_attempt() {
    let error = listing_error(StubAnswer::Fail(TransportError::Timeout {
        detail: "no answer in 60s".to_owned(),
    }))
    .await;

    assert!(matches!(error, Error::Timeout { .. }), "{error:?}");
    assert!(error.is_retryable());
}

#[tokio::test]
async fn a_call_that_never_landed_is_worth_another_attempt() {
    let error = listing_error(StubAnswer::Fail(TransportError::Connect {
        detail: "connection refused".to_owned(),
    }))
    .await;

    assert!(matches!(error, Error::Transport { .. }), "{error:?}");
    assert!(error.is_retryable());
}

#[tokio::test]
async fn a_genuine_refusal_is_not_worth_repeating() {
    let error = listing_error(StubAnswer::json(
        403,
        &envelope("insufficientFilePermissions"),
    ))
    .await;

    assert!(matches!(error, Error::PermissionDenied { .. }), "{error:?}");
    assert!(!error.is_retryable());
}

#[tokio::test]
async fn a_missing_object_is_not_worth_asking_for_again() {
    let error = listing_error(StubAnswer::json(404, &envelope("notFound"))).await;

    assert!(matches!(error, Error::NotFound { .. }), "{error:?}");
    assert!(!error.is_retryable());
}

#[tokio::test]
async fn a_request_drive_will_never_accept_is_not_repeated() {
    let error = listing_error(StubAnswer::json(400, &envelope("invalid"))).await;

    assert!(
        matches!(error, Error::Rejected { status: 400, .. }),
        "{error:?}"
    );
    assert!(!error.is_retryable());
}
