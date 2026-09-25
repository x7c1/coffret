//! Which failures are worth another attempt.
//!
//! A storage layer that cannot tell throttling from refusal either gives up on
//! a Library it is entitled to, or hammers an endpoint that will never say yes.
//! Drive reports both as a 403, and both as prose in a message — so the
//! classification happens once, here, and everything above works from the type.
//!
//! None of these can be provoked against the real API on demand, which is the
//! whole reason the transport is injected.

use std::io;
use std::sync::Arc;

use coffret_usecase::{Error, ObjectStore};

use coffret_model::Redacted;

use crate::api::FailedResponse;
use crate::http::{StubAnswer, TransportError};
use crate::test_support::{chain, scripted_drive};

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
        cause: Arc::new(io::Error::new(io::ErrorKind::TimedOut, "no answer in 60s")),
    }))
    .await;

    assert!(matches!(error, Error::Timeout { .. }), "{error:?}");
    assert!(error.is_retryable());
}

#[tokio::test]
async fn a_call_that_never_landed_is_worth_another_attempt() {
    let error = listing_error(StubAnswer::Fail(TransportError::Connect {
        cause: Arc::new(io::Error::from(io::ErrorKind::ConnectionRefused)),
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
async fn a_limit_drive_has_reached_is_told_apart_from_access_it_refused() {
    // All three arrive as a 403, the same status a missing permission does, and
    // none of them is one: the Drive is full, the account holds as many items
    // as Drive allows, or one folder does.
    for reason in [
        "storageQuotaExceeded",
        "activeItemCreationLimitExceeded",
        "numChildrenInNonRootLimitExceeded",
    ] {
        let error = listing_error(StubAnswer::json(403, &envelope(reason))).await;

        let Error::LimitReached { limit, .. } = &error else {
            panic!("{reason} is a limit reached, not access refused: {error:?}");
        };
        assert_eq!(limit, reason, "which limit it was is what a person acts on");
        assert!(
            !error.is_retryable(),
            "{reason} stays reached however long anyone waits",
        );
    }
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

// What crosses the port beside the variant. The value the gateway classified
// the failure from is the port error's `source`, and a person reads it as the
// chain under the port's line. `detail` is a sentence for a caller that reads
// the field instead, and a diagnostic event renders neither the one nor the
// other.
// One failure of each kind the gateway meets, walked from the port down.

#[tokio::test]
async fn a_transport_break_crosses_the_port_as_the_value_it_was() {
    let error = listing_error(StubAnswer::Fail(TransportError::Connect {
        cause: Arc::new(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            "connection refused by storage.invalid",
        )),
    }))
    .await;

    let Error::Transport { detail, .. } = &error else {
        panic!("a call that never landed is a transport failure: {error:?}");
    };
    let transport = std::error::Error::source(&error)
        .and_then(|below| below.downcast_ref::<TransportError>())
        .expect("the transport's own error is the next link");
    assert!(
        matches!(transport, TransportError::Connect { .. }),
        "{transport:?}"
    );
    let reported = std::error::Error::source(transport)
        .and_then(|below| below.downcast_ref::<io::Error>())
        .expect("and what the transport reported is the one under it, as itself");
    assert_eq!(reported.kind(), io::ErrorKind::ConnectionRefused);

    // Each link says what its own layer knows and nothing a link beside it
    // says: the port its kind of failure, the transport which call it was, the
    // operating system what refused. No sentence reaches a person twice.
    let links = chain(&error);
    assert_eq!(
        links,
        vec![
            "could not reach Storage".to_owned(),
            "the call could not be made".to_owned(),
            "connection refused by storage.invalid".to_owned(),
        ],
    );
    assert_no_link_repeats_another(&links);
    assert_eq!(detail, "the call could not be made");

    assert_eq!(error.redacted(), "Storage::Transport");
}

/// Fails where one link's text is contained in another's, which is a sentence
/// a chain printed link by link would show twice.
fn assert_no_link_repeats_another(links: &[String]) {
    for (i, outer) in links.iter().enumerate() {
        for (j, inner) in links.iter().enumerate() {
            assert!(
                i == j || !outer.contains(inner.as_str()),
                "{inner:?} is said again inside {outer:?}: {links:?}"
            );
        }
    }
}

#[tokio::test]
async fn a_refusal_crosses_the_port_with_what_drive_answered_as_fields() {
    let error = listing_error(StubAnswer::json(
        403,
        r#"{"error":{"message":"The user does not have sufficient permissions for file head-1.cfrt.","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    ))
    .await;

    assert!(
        matches!(error, Error::PermissionDenied { .. }),
        "a 403 naming no limit and no throttling is access refused: {error:?}"
    );
    let answered = std::error::Error::source(&error)
        .and_then(|below| below.downcast_ref::<FailedResponse>())
        .expect("what Drive answered is the next link, whole");
    assert_eq!(
        answered.to_string(),
        "Drive answered list with status 403 and reason insufficientFilePermissions: \
         The user does not have sufficient permissions for file head-1.cfrt.",
    );
    assert!(std::error::Error::source(answered).is_none());
    assert_no_link_repeats_another(&chain(&error));

    assert_eq!(error.redacted(), "Storage::PermissionDenied");
}

#[tokio::test]
async fn a_defect_the_gateway_composed_itself_has_nothing_behind_it() {
    let error = listing_error(StubAnswer::json(200, r#"{"files":[{"id":"1a2B3c"}]}"#)).await;

    assert!(
        matches!(error, Error::MalformedResponse { .. }),
        "a listed file without a name is an answer this build cannot read: {error:?}"
    );
    assert!(
        std::error::Error::source(&error).is_none(),
        "the sentence is the gateway's own, and nothing stands behind it: {error:?}",
    );

    assert_eq!(error.redacted(), "Storage::MalformedResponse");
}
