//! Reaching an S3 bucket from what a device recorded about it.
//!
//! Three callers, one client. Opening a Library builds a store over its prefix,
//! creating or joining one asks the bucket whether it is there at all, and
//! joining one asks the prefix whether a Library is under it; all three address
//! the bucket through [`client`], and a second assembly of endpoint, region and
//! addressing style would be a second answer able to disagree with the first.
//! The checks take the client rather than build one, so a join that asks both
//! questions resolves the configuration and the credentials once.
//!
//! No credential comes from the settings, here or anywhere: the SDK resolves
//! them the way it resolves them for everything else — the environment, then a
//! profile — so a device that may reach the bucket does, and one that may not is
//! refused by S3 rather than by a file this crate wrote.

use aws_config::BehaviorVersion;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::Client;

use crate::error::{Error, Result};

/// A client addressed at the bucket's endpoint, region, and addressing style.
pub(crate) async fn client(
    endpoint: Option<&str>,
    region: Option<&str>,
    path_style: bool,
) -> Client {
    // The timeouts every call shares are stated rather than left to the
    // behavior version's defaults; the per-call deadlines are put on by the
    // gateway as each call is sent (see `s3_store::call_deadline`).
    let mut loader = aws_config::defaults(BehaviorVersion::latest())
        .timeout_config(s3_store::timeout_config())
        .stalled_stream_protection(s3_store::stalled_stream_protection());
    if let Some(region) = region {
        loader = loader.region(Region::new(region.to_owned()));
    }
    // This crate's own cases address a loopback stub over plain HTTP, and the
    // SDK's default client reads the operating system's root certificates as
    // it builds its TLS half whether or not the endpoint is `https`. On macOS
    // that read comes back empty often enough to fail a case — the SDK
    // asserts on it in a debug build — with nothing about the case at fault.
    // So the cases reach the stub through a client with no TLS half at all,
    // which is the one thing an `http://` loopback needs; what ships keeps the
    // SDK's own.
    #[cfg(test)]
    {
        loader = loader.http_client(aws_smithy_http_client::Builder::new().build_http());
    }
    let resolved = loader.load().await;

    let mut config = aws_sdk_s3::config::Builder::from(&resolved).force_path_style(path_style);
    if let Some(endpoint) = endpoint {
        config = config.endpoint_url(endpoint);
    }
    Client::from_conf(config.build())
}

/// Asks the bucket whether it is there, and refuses the Library if it is not.
///
/// The one call creating a Library on S3 makes to Storage, and it exists
/// because otherwise there would be none. A join makes it too and then asks
/// [`check_any_head_or_snapshot`] a second thing, which is a question about the
/// prefix rather than about the bucket. On S3 a prefix exists by being written
/// under, so nothing about setting a Library up would notice a mistyped bucket,
/// an endpoint nothing is listening at, or credentials the SDK could not
/// resolve: all three would be answered by a complete "success" and a Recovery
/// Code, and found out at the first sync instead (spec: FM-18).
///
/// Which of the three it was is the gateway's to decide: the question goes out
/// through [`s3_store::check_bucket`], so the answer arrives already classified
/// in the Storage port's vocabulary and this crate only says which bucket it was
/// about. Reading an S3 status here would be a second copy of a table that
/// already exists one layer down, free to disagree with it.
pub(crate) async fn check_bucket(client: &Client, bucket: &str) -> Result<()> {
    s3_store::check_bucket(client, bucket)
        .await
        .map_err(|cause| Error::BucketUnreachable {
            bucket: bucket.to_owned(),
            cause,
        })
}

/// Whether the Library's prefix holds any head or Index Snapshot.
///
/// The one question taking up an existing S3 Library puts to Storage beyond
/// whether the bucket is there. A prefix is typed rather than minted, and on S3
/// it comes into being by being written under, so nothing about its shape says
/// whether the Library it names has ever existed: a Library ID with one
/// character wrong is a perfectly good prefix that holds nothing. Any head or
/// Snapshot rather than one named object, because which of them survive
/// depends on what has been pruned (see [`FoundOnStorage`](crate::FoundOnStorage)).
///
/// The answer is a `bool` because absence is not a refusal — a Library created
/// and never synced holds nothing either, and the two cannot be told apart from
/// here (spec: FM-18). Everything that is not an answer about the prefix — a
/// bucket that is not there, credentials S3 refused, an endpoint nothing is
/// listening at, a listing that never ends — arrives as
/// [`Error::BucketUnreachable`], which is the same verdict [`check_bucket`]
/// makes of the same causes: this device cannot use that bucket, and the
/// gateway's classification says which of them it was.
pub(crate) async fn check_any_head_or_snapshot(
    client: &Client,
    bucket: &str,
    prefix: &str,
) -> Result<bool> {
    s3_store::check_any_head_or_snapshot(client, bucket, prefix)
        .await
        .map_err(|cause| Error::BucketUnreachable {
            bucket: bucket.to_owned(),
            cause,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The gateway's cases build their own client from the same figures, so
    // what only this can show is that the settings survive the way the device
    // assembles one — through the shared config and the S3 builder made from
    // it — rather than being dropped on the way and replaced by the SDK's
    // defaults.
    #[tokio::test]
    async fn the_client_carries_the_gateway_s_timeouts() {
        let client = client(Some("http://127.0.0.1:1"), Some("us-east-1"), true).await;
        let config = client.config();

        let timeouts = config.timeout_config().expect("timeouts are stated");
        assert_eq!(timeouts.connect_timeout(), Some(s3_store::CONNECT_TIMEOUT));
        assert_eq!(timeouts.read_timeout(), None);
        assert_eq!(timeouts.operation_timeout(), None);
        assert_eq!(timeouts.operation_attempt_timeout(), None);

        let stall = config
            .stalled_stream_protection()
            .expect("stall protection is stated");
        assert!(stall.upload_enabled() && stall.download_enabled());
        assert_eq!(stall.grace_period(), s3_store::STALL_GRACE_PERIOD);
    }
}
