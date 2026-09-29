//! The deadlines on whole calls, against an endpoint that never answers.
//!
//! A stub that takes the connection and then says nothing is the shape of a
//! filtered network or a wedged endpoint, and it is the one no other bound
//! catches: the connection was made, and no transfer is running for the
//! stalled-stream protection to watch. The client here is built the way the
//! device builds one — this crate's timeouts and stall protection — with a
//! plain-HTTP connector for the loopback, and the store is handed a deadline
//! short enough to wait out.

use std::time::Duration;

use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::Client;
use coffret_usecase::{ByteStream, Error, ObjectStore};
use s3_store::{S3Settings, S3};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::time::Instant;

/// The deadline the case holds small calls to.
const DEADLINE: Duration = Duration::from_millis(300);

/// Starts a listener that takes every connection and every request on it, and
/// answers none of them. Returns the endpoint it listens at.
async fn silent_endpoint() -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port is free");
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                // Drained so the client is never held up sending, and never
                // answered.
                let mut buf = [0u8; 4096];
                while matches!(socket.read(&mut buf).await, Ok(n) if n > 0) {}
            });
        }
    });
    endpoint
}

/// A client built with this crate's timeouts, pointed at `endpoint`.
fn client_at(endpoint: &str) -> Client {
    let config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .endpoint_url(endpoint)
        .credentials_provider(Credentials::new("key", "secret", None, None, "test"))
        .force_path_style(true)
        .timeout_config(s3_store::timeout_config())
        .stalled_stream_protection(s3_store::stalled_stream_protection())
        .http_client(aws_smithy_http_client::Builder::new().build_http())
        .build();
    Client::from_conf(config)
}

#[tokio::test]
async fn a_listing_nothing_answers_times_out_at_the_deadline() {
    let endpoint = silent_endpoint().await;
    let store = S3::new(
        client_at(&endpoint),
        S3Settings::new("bucket")
            .with_prefix("libraries/alpha/")
            .with_small_call_deadline(DEADLINE),
    );

    let started = Instant::now();
    let result = store.list(None).await;
    let took = started.elapsed();

    let error = result.expect_err("nothing answered, so nothing was listed");
    assert!(
        matches!(error, Error::Timeout { .. }),
        "silence is a timeout: {error:?}"
    );
    // The same classification every other timeout gets, and so one the retry
    // policy makes again.
    assert!(error.is_retryable(), "{error}");
    assert!(took >= DEADLINE, "it waited the deadline out: {took:?}");
    assert!(
        took < Duration::from_secs(5),
        "it ended at the deadline, not at some other bound: {took:?}"
    );
}

// The gap a stall protection alone would leave: the body goes up, every byte of
// it, and then nothing answers. A small body is held to the deadline itself, so
// the case waits out the shortened one.
#[tokio::test]
async fn an_upload_whose_answer_never_comes_times_out_at_the_deadline() {
    let endpoint = silent_endpoint().await;
    let store = S3::new(
        client_at(&endpoint),
        S3Settings::new("bucket")
            .with_prefix("libraries/alpha/")
            .with_small_call_deadline(DEADLINE),
    );

    let started = Instant::now();
    let result = store
        .put(
            "0123456789abcdef0123456789abcdef.cfrt",
            ByteStream::from(b"ciphertext".to_vec()),
        )
        .await;
    let took = started.elapsed();

    let error = result.expect_err("nothing answered, so nothing was stored");
    assert!(
        matches!(error, Error::Timeout { .. }),
        "an answer that never comes is a timeout: {error:?}"
    );
    assert!(error.is_retryable(), "{error}");
    assert!(took >= DEADLINE, "it waited the deadline out: {took:?}");
    assert!(
        took < Duration::from_secs(5),
        "it ended at the deadline, not at some other bound: {took:?}"
    );
}
