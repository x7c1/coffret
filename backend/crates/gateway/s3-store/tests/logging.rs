//! What reaches the log when S3 answers something the port has no state for.
//!
//! The same rule as the other gateway, proved the same way: a refusal that maps
//! to a catch-all is recorded with the status, the code, and the body it
//! arrived in, because nothing above this crate will ever see any of them — and
//! a key that holds nothing is ordinary rather than an error.
//!
//! Answered from a replayed response rather than a bucket, so the cases run
//! wherever the suite does.

use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::Client;
use aws_smithy_runtime::client::http::test_util::{ReplayEvent, StaticReplayClient};
use aws_smithy_runtime_api::client::orchestrator::{HttpRequest, HttpResponse};
use aws_smithy_runtime_api::http::StatusCode;
use aws_smithy_types::body::SdkBody;
use coffret_logging::testing::CapturedLogs;
use coffret_usecase::{ByteStream, Error, ObjectRef, ObjectStore};
use s3_store::{S3Settings, S3};
use tracing::Level;

/// S3's refusal of a caller whose credentials do not reach the bucket.
const NO_PERMISSION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>AccessDenied</Code><Message>Access Denied</Message></Error>"#;

/// What S3 answers for a key holding nothing.
const NO_SUCH_KEY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>NoSuchKey</Code><Message>The specified key does not exist.</Message></Error>"#;

/// A store whose every call is answered with this refusal.
fn refusing_store(status: u16, body: &'static str) -> S3 {
    let response = HttpResponse::new(
        StatusCode::try_from(status).expect("a test uses real statuses"),
        SdkBody::from(body),
    );
    let http_client = StaticReplayClient::new(vec![ReplayEvent::new(
        HttpRequest::new(SdkBody::empty()),
        response,
    )]);

    let config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .endpoint_url("http://storage.invalid")
        .credentials_provider(Credentials::new("key", "secret", None, None, "test"))
        .force_path_style(true)
        .http_client(http_client)
        .build();

    S3::new(
        Client::from_conf(config),
        S3Settings::new("bucket").with_prefix("libraries/alpha"),
    )
}

#[tokio::test]
async fn a_refusal_the_port_has_no_state_for_is_recorded_as_it_arrived() {
    let logs = CapturedLogs::capture_target("s3_store");
    let store = refusing_store(403, NO_PERMISSION);

    let error = store
        .put("head-1.cfrt", ByteStream::from(b"ciphertext".to_vec()))
        .await
        .expect_err("a refused write must fail");
    assert!(matches!(error, Error::PermissionDenied { .. }), "{error:?}");

    let event = logs.only(Level::WARN);
    assert_eq!(event.field("operation"), "put");
    assert_eq!(event.number("status"), 403);
    assert_eq!(event.field("reason"), "AccessDenied");
    assert!(
        event.field("body").contains("Access Denied"),
        "the body is the evidence: {event}",
    );
}

#[tokio::test]
async fn a_key_that_holds_nothing_is_not_a_failure_anybody_has_to_act_on() {
    let logs = CapturedLogs::capture_target("s3_store");
    let store = refusing_store(404, NO_SUCH_KEY);

    let error = store
        .get(&ObjectRef::new("head-1.cfrt"), None)
        .await
        .expect_err("a missing key must fail the read");
    assert!(matches!(error, Error::NotFound { .. }), "{error:?}");

    assert!(logs.at(Level::ERROR).is_empty(), "{}", logs.text());
    assert!(logs.at(Level::WARN).is_empty(), "{}", logs.text());
}

#[tokio::test]
async fn an_object_that_reached_storage_is_recorded_as_progress() {
    let logs = CapturedLogs::capture_target("s3_store");
    let store = refusing_store(200, "");

    store
        .put("head-1.cfrt", ByteStream::from(b"ciphertext".to_vec()))
        .await
        .expect("the write must succeed");

    let event = logs.only(Level::INFO);
    assert_eq!(event.message(), "stored an object");
    assert_eq!(event.field("object"), "head-1.cfrt");
    assert_eq!(event.number("bytes"), 10);
}

#[tokio::test]
async fn an_individual_call_is_detail_rather_than_progress() {
    let logs = CapturedLogs::capture_target("s3_store");
    let store = refusing_store(200, "");

    store
        .put("head-1.cfrt", ByteStream::from(b"ciphertext".to_vec()))
        .await
        .expect("the write must succeed");

    let event = logs.only(Level::DEBUG);
    assert_eq!(event.field("operation"), "put");
    // Which S3 call the operation turned into, and the key it addressed. No
    // status: the SDK owns the request, and a successful output carries none
    // back out of it — so the event says what this crate knows and no more.
    assert_eq!(event.field("call"), "put_object");
    assert_eq!(event.field("key"), "libraries/alpha/head-1.cfrt");
}

#[tokio::test]
async fn no_credential_reaches_the_log() {
    let logs = CapturedLogs::capture_target("s3_store");
    let store = refusing_store(403, NO_PERMISSION);

    let _ = store
        .put("head-1.cfrt", ByteStream::from(b"ciphertext".to_vec()))
        .await;

    // Every request the SDK sends is signed with these, and the object's bytes
    // went through the same call: none of it may reach an event of ours. What
    // the SDK writes about itself is its own business, which is why only this
    // crate's events are captured — and why the installed sink never runs above
    // `DEBUG`, the level below which only dependencies emit.
    logs.assert_free_of(&["secret", "AWS4-HMAC", "Signature=", "ciphertext"]);
}

#[tokio::test]
async fn emitting_with_nothing_installed_changes_nothing() {
    let store = refusing_store(403, NO_PERMISSION);
    let without = store
        .put("head-1.cfrt", ByteStream::from(b"ciphertext".to_vec()))
        .await
        .expect_err("a refused write must fail");

    let logs = CapturedLogs::capture_target("s3_store");
    let store = refusing_store(403, NO_PERMISSION);
    let with = store
        .put("head-1.cfrt", ByteStream::from(b"ciphertext".to_vec()))
        .await
        .expect_err("a refused write must fail");

    // Compared by what the failure means — the variant, and the detail that
    // travels in it — rather than by the whole value: an error type is not
    // asked whether it is equal to another, so that a field added to it later
    // is not a change to what these two runs are being held to.
    match (without, with) {
        (Error::PermissionDenied { detail: without }, Error::PermissionDenied { detail: with }) => {
            assert_eq!(without, with)
        }
        (without, with) => {
            panic!("the refusal did not survive being recorded: {without:?} became {with:?}")
        }
    }
    assert!(
        !logs.text().is_empty(),
        "the path has to emit something, or this case proves nothing",
    );
}
