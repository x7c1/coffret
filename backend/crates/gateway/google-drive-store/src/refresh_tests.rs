//! What a rejected token costs.
//!
//! A grant can be refreshed out from under a running process at any moment, so
//! one 401 has to be answered by minting a token and trying again. Exactly
//! once: a grant that was revoked answers 401 to every token there will ever
//! be, and a gateway that kept refreshing would spin against it forever.

use coffret_usecase::{Error, ObjectStore};

use crate::http::StubAnswer;
use crate::test_support::scripted_drive;

/// A listing Drive is happy to serve.
const EMPTY_LISTING: &str = r#"{"files":[]}"#;

/// Drive's answer to a token it will not accept.
const REJECTED: &str =
    r#"{"error":{"message":"Invalid Credentials","errors":[{"reason":"authError"}]}}"#;

#[tokio::test]
async fn a_rejected_token_is_refreshed_once_and_the_call_retried() {
    let (store, transport, tokens) = scripted_drive([
        StubAnswer::json(401, REJECTED),
        StubAnswer::json(200, EMPTY_LISTING),
    ]);

    let page = store
        .list(None)
        .await
        .expect("the retry after a refresh must succeed");

    assert!(page.objects.is_empty());
    assert_eq!(tokens.refresh_count(), 1);
    assert_eq!(transport.call_count(), 2);
    assert_eq!(
        transport.request(1).header("authorization"),
        Some("Bearer token-1"),
        "the retry must carry the token that was just minted"
    );
}

#[tokio::test]
async fn a_second_rejection_ends_the_call_rather_than_the_refreshing() {
    let (store, transport, tokens) = scripted_drive([
        StubAnswer::json(401, REJECTED),
        StubAnswer::json(401, REJECTED),
    ]);

    let error = store
        .list(None)
        .await
        .expect_err("a grant that is gone must fail the call");

    assert!(matches!(error, Error::Unauthenticated { .. }), "{error:?}");
    assert!(!error.is_retryable());
    assert_eq!(
        tokens.refresh_count(),
        1,
        "the token must be refreshed exactly once"
    );
    assert_eq!(
        transport.call_count(),
        2,
        "the call must not be made a third time"
    );
}

#[tokio::test]
async fn a_token_drive_accepts_is_not_refreshed() {
    let (store, transport, tokens) = scripted_drive([StubAnswer::json(200, EMPTY_LISTING)]);

    store.list(None).await.expect("the call must succeed");

    assert_eq!(tokens.refresh_count(), 0);
    assert_eq!(transport.call_count(), 1);
    assert_eq!(
        transport.request(0).header("authorization"),
        Some("Bearer token-0")
    );
}
