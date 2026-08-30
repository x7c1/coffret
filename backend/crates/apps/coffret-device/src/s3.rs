//! Reaching an S3 bucket from what a device recorded about it.
//!
//! Two callers, one client. Opening a Library builds a store over its prefix,
//! and creating or joining one asks the bucket whether it is there at all; both
//! address the bucket the same way, and a second assembly of endpoint, region
//! and addressing style would be a second answer able to disagree with the
//! first.
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
    let mut loader = aws_config::defaults(BehaviorVersion::latest());
    if let Some(region) = region {
        loader = loader.region(Region::new(region.to_owned()));
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
/// The one call a Library's first moments on S3 make to Storage, and it exists
/// because otherwise there would be none. On S3 a prefix exists by being written
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
pub(crate) async fn check_bucket(
    bucket: &str,
    endpoint: Option<&str>,
    region: Option<&str>,
    path_style: bool,
) -> Result<()> {
    let client = client(endpoint, region, path_style).await;
    s3_store::check_bucket(&client, bucket)
        .await
        .map_err(|cause| Error::BucketUnreachable {
            bucket: bucket.to_owned(),
            cause,
        })
}
