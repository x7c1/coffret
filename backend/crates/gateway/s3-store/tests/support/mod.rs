//! A store whose requests are captured instead of sent.
//!
//! Shared by the cases that ask what this gateway puts on the wire — and by the
//! ones that ask whether it puts anything there at all.

use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::Client;
use aws_smithy_runtime::client::http::test_util::{capture_request, CaptureRequestReceiver};
use s3_store::{S3Settings, S3};

/// Builds a store pointed at nowhere, and the receiver holding what it sent.
///
/// The endpoint is unroutable on purpose: a case that expects no request would
/// otherwise be one DNS entry away from proving nothing.
pub fn captured_store() -> (CaptureRequestReceiver, S3) {
    let (http_client, sent) = capture_request(None);
    let config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .endpoint_url("http://storage.invalid")
        .credentials_provider(Credentials::new("key", "secret", None, None, "test"))
        .force_path_style(true)
        .http_client(http_client)
        .build();

    let store = S3::new(
        Client::from_conf(config),
        S3Settings::new("bucket").with_prefix("libraries/alpha"),
    );
    (sent, store)
}
