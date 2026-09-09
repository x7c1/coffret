//! What reaches the log when S3 answers something the port has no state for.
//!
//! The same rule as the other gateway, proved the same way: a refusal that maps
//! to a catch-all is recorded with the status, the code, and the body, because
//! nothing above this crate will ever see any of them — and a key that holds
//! nothing is ordinary rather than an error.
//!
//! Answered from a replayed response rather than a bucket, so the cases run
//! wherever the suite does.

use aws_sdk_s3::config::retry::RetryConfig;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::Client;
use aws_smithy_http_client::test_util::{ReplayEvent, StaticReplayClient};
use aws_smithy_runtime_api::client::orchestrator::{HttpRequest, HttpResponse};
use aws_smithy_runtime_api::http::StatusCode;
use aws_smithy_types::body::SdkBody;
use coffret_logging::testing::CapturedLogs;
use coffret_model::Redacted;
use coffret_usecase::{ByteStream, Error, ObjectRef, ObjectStore, RetryPolicy};
use s3_store::{S3Settings, S3};
use std::time::Duration;
use tracing::Level;

/// S3's refusal of a caller whose credentials do not reach the bucket.
const NO_PERMISSION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>AccessDenied</Code><Message>Access Denied</Message></Error>"#;

/// What S3 answers for a key holding nothing.
const NO_SUCH_KEY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>NoSuchKey</Code><Message>The specified key does not exist.</Message></Error>"#;

/// A prefix recognizable as this device's private configuration.
const PRIVATE_PREFIX: &str = "people/alice/Summer Library";

/// A successful empty listing. The response echoes the requested prefix, as a
/// real S3 listing does, but the adapter does not put response metadata into an
/// event.
const EMPTY_LIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Name>bucket</Name><Prefix>people/alice/Summer Library/</Prefix><KeyCount>0</KeyCount><MaxKeys>1000</MaxKeys><IsTruncated>false</IsTruncated></ListBucketResult>"#;

/// A provider failure that echoes both private request data and a credential.
const ECHOED_PREFIX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>UnexpectedListing</Code><Message>Could not list people/alice/Summer Library/</Message><Detail>Authorization: Bearer provider-token</Detail></Error>"#;

/// What S3 answers for a listing of a prefix in no bucket of its own, echoing
/// the prefix it was asked for.
const NO_SUCH_LISTING: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>NoSuchBucket</Code><Message>No listing at people/alice/Summer Library/</Message></Error>"#;

/// A store whose every call is answered with this refusal.
fn refusing_store(status: u16, body: &'static str) -> S3 {
    store_with_prefix(status, body, "libraries/alpha")
}

/// A store whose configured prefix is part of the case's evidence.
fn store_with_prefix(status: u16, body: &'static str, prefix: &str) -> S3 {
    answering_store(prefix, [(status, body)])
}

/// A store answered in order by a deterministic HTTP fixture.
fn answering_store<const N: usize>(prefix: &str, answers: [(u16, &'static str); N]) -> S3 {
    let events = answers
        .into_iter()
        .map(|(status, body)| {
            ReplayEvent::new(
                HttpRequest::new(SdkBody::empty()),
                HttpResponse::new(
                    StatusCode::try_from(status).expect("a test uses real statuses"),
                    SdkBody::from(body),
                ),
            )
        })
        .collect();
    let http_client = StaticReplayClient::new(events);

    let config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .endpoint_url("http://storage.invalid")
        .credentials_provider(Credentials::new("key", "secret", None, None, "test"))
        .retry_config(RetryConfig::disabled())
        .force_path_style(true)
        .http_client(http_client)
        .build();

    S3::new(
        Client::from_conf(config),
        S3Settings::new("bucket").with_prefix(prefix),
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
    // Which S3 call the operation turned into, and the object it addressed. No
    // status: the SDK owns the request, and a successful output carries none
    // back out of it — so the event says what this crate knows and no more.
    assert_eq!(event.field("call"), "put_object");
    // The name coffret minted, and not the key it was stored under: a key
    // begins with the prefix the store was configured with, which is
    // somebody's configuration rather than an object identifier (spec: EL-5).
    assert_eq!(event.field("object"), "head-1.cfrt");
    logs.assert_free_of(&["libraries/alpha"]);
}

#[tokio::test]
async fn a_deletion_that_did_not_take_records_which_half_kept_the_object() {
    let logs = CapturedLogs::capture_target("s3_store");
    // Both deletions answered; then the live key still holds something and the
    // trashed one does not.
    let store = answering_store(
        PRIVATE_PREFIX,
        [(200, ""), (200, ""), (200, ""), (404, NO_SUCH_KEY)],
    );

    let error = store
        .purge(&ObjectRef::new("head-1.cfrt"))
        .await
        .expect_err("an unconfirmed deletion must fail");
    assert!(matches!(error, Error::NotPurged { .. }), "{error:?}");

    let event = logs.only(Level::WARN);
    assert_eq!(event.field("operation"), "purge");
    assert_eq!(event.field("object"), "head-1.cfrt");
    // Which half of the key space kept it, because none of the calls above can
    // say: a live key and a trashed one are recorded by the one name they
    // share. Live and trashed are this gateway's own layout, so naming them
    // costs nothing the configured location would (spec: EL-5).
    assert_eq!(event.field("live"), "true");
    assert_eq!(event.field("trashed"), "false");
    logs.assert_free_of(&[PRIVATE_PREFIX, "Summer Library", "trash/"]);
}

#[tokio::test]
async fn a_successful_listing_records_the_call_without_its_private_prefix() {
    let logs = CapturedLogs::capture_target("s3_store");
    let store = store_with_prefix(200, EMPTY_LIST, PRIVATE_PREFIX);

    let page = store.list(None).await.expect("an empty listing succeeds");
    assert!(page.objects.is_empty());

    let event = logs.only(Level::DEBUG);
    assert_eq!(event.field("operation"), "list");
    assert_eq!(event.field("call"), "list_objects_v2");
    logs.assert_free_of(&[PRIVATE_PREFIX, "Summer Library"]);
}

#[tokio::test]
async fn a_missing_listing_uses_a_fixed_diagnostic_subject() {
    let logs = CapturedLogs::capture_target("s3_store");
    let store = store_with_prefix(404, NO_SUCH_LISTING, PRIVATE_PREFIX);

    let error = store
        .list(None)
        .await
        .expect_err("a missing listing must be reported");
    let Error::NotFound { object } = &error else {
        panic!("the missing listing must be not found: {error:?}");
    };
    assert_eq!(object, "the listing");
    assert_eq!(
        error.redacted(),
        "no object named \"the listing\" in Storage"
    );

    let event = logs.only(Level::DEBUG);
    assert_eq!(event.field("operation"), "list");
    assert_eq!(event.field("object"), "the listing");
    logs.assert_free_of(&[PRIVATE_PREFIX, "Summer Library"]);
}

#[tokio::test]
async fn a_provider_echo_is_redacted_without_losing_listing_evidence() {
    let logs = CapturedLogs::capture_target("s3_store");
    let store = store_with_prefix(418, ECHOED_PREFIX, PRIVATE_PREFIX);

    let error = store
        .list(None)
        .await
        .expect_err("the provider refused the listing");
    let rendered = error.redacted();
    assert!(rendered.contains("status 418"), "{rendered}");
    assert!(rendered.contains("Could not list"), "{rendered}");

    let event = logs.only(Level::WARN);
    assert_eq!(event.field("operation"), "list");
    assert_eq!(event.number("status"), 418);
    assert_eq!(event.field("reason"), "UnexpectedListing");
    assert!(event.field("body").contains("Could not list"), "{event}");
    assert!(event.field("body").contains("[redacted]"), "{event}");
    logs.assert_free_of(&[
        PRIVATE_PREFIX,
        "Summer Library",
        "provider-token",
        "Bearer provider-token",
    ]);
    assert!(!rendered.contains(PRIVATE_PREFIX), "{rendered}");
    assert!(!rendered.contains("provider-token"), "{rendered}");
}

#[tokio::test]
async fn retry_give_up_keeps_provider_evidence_without_the_private_prefix() {
    // The installed sink keeps `coffret_*` and provider gateway targets by
    // default and excludes the AWS SDK's own traces. Capture the retry target
    // under that same boundary: dependency instrumentation follows no coffret
    // diagnostic contract and appears only when an operator explicitly opts in.
    let logs = CapturedLogs::capture_target("coffret_usecase");
    let store = answering_store(PRIVATE_PREFIX, [(503, ECHOED_PREFIX), (503, ECHOED_PREFIX)]);
    let policy = RetryPolicy::default()
        .with_attempts(2)
        .with_base_backoff(Duration::from_millis(1))
        .with_wait_ceiling(Duration::from_millis(1))
        .with_total_wait(Duration::from_secs(1));

    let error = policy
        .run("list", || store.list(None))
        .await
        .expect_err("both listing attempts fail");

    let event = logs.only(Level::WARN);
    assert!(event.message().contains("gave up"), "{event}");
    assert_eq!(event.field("operation"), "list");
    assert_eq!(event.number("attempts"), 2);
    assert!(event.field("error").contains("status 503"), "{event}");
    assert!(event.field("error").contains("Could not list"), "{event}");
    logs.assert_free_of(&[PRIVATE_PREFIX, "Summer Library", "provider-token"]);
    let rendered = error.redacted();
    assert!(!rendered.contains(PRIVATE_PREFIX), "{rendered}");
    assert!(!rendered.contains("provider-token"), "{rendered}");
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
