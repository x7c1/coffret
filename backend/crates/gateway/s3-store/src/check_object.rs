//! Asking a bucket whether one object is under a prefix, before there is a
//! store over it.
//!
//! The sibling of [`check_bucket`](crate::check_bucket) and here for the same
//! reason: taking up a Library that already exists has one question for Storage
//! that it has to ask before a store is built, and reading the answer means
//! reading a status and an S3 error code — which this crate holds the one table
//! for.
//!
//! Where a Library lives on S3 is a prefix somebody typed, and a prefix that is
//! not a Library's looks exactly like one: keys come into being by being
//! written, so nothing about the shape of a prefix says whether any Library is
//! there. That is what this answers — whether the object a caller names stands
//! under the prefix — and it is deliberately a `bool` rather than a refusal.
//! A Library that has been created and never synced holds nothing either, so
//! absence is two different things and neither of them is an error.

use aws_sdk_s3::Client;
use coffret_logging::redact::PrivateValues;
use coffret_usecase::{Error, Result};

use crate::error::classify_object;
use crate::key_layout::KeyLayout;

/// What the call is recorded and reported as.
const OPERATION: &str = "check_object";

/// Whether `bucket` holds an object called `name` under `prefix`.
///
/// One `HeadObject`, and the two answers a caller acts on are a `bool`: the
/// object is there, or S3 answered about the bucket and does not hold it.
/// Everything else — credentials that were refused, an endpoint nothing is
/// listening at, a bucket that is not there — travels as the port's own error,
/// because those say nothing about the prefix and a caller reading them as
/// "no Library here" would be reporting the wrong thing entirely.
///
/// The prefix goes through the same key layout every other call in this crate
/// uses, so the key asked about is the key the store would read.
pub async fn check_object(client: &Client, bucket: &str, prefix: &str, name: &str) -> Result<bool> {
    let key = KeyLayout::new(prefix).live_key(name);
    // S3 answers a refusal by quoting back what it was asked about — in prose,
    // in the URI, or both — so the bucket and the key are taken out of whatever
    // provider text the failure carries (spec: EL-5). The key carries the
    // Library's own prefix, which is somebody's configuration.
    let private = PrivateValues::none().with(bucket).with(&key);
    match client
        .head_object()
        .bucket(bucket)
        .key(&key)
        .send()
        .await
        .map_err(|error| classify_object(OPERATION, name, error, &private))
    {
        Ok(_) => Ok(true),
        Err(Error::NotFound { .. }) => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use aws_sdk_s3::config::retry::RetryConfig;
    use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
    use aws_smithy_http_client::test_util::{ReplayEvent, StaticReplayClient};
    use aws_smithy_runtime_api::client::orchestrator::{HttpRequest, HttpResponse};
    use aws_smithy_types::body::SdkBody;

    use super::*;

    /// The bucket the cases ask about.
    const BUCKET: &str = "someones-holiday-photos";

    /// The prefix a Library of its own would sit under.
    const PREFIX: &str = "archive/coffret-0123456789abcdef/";

    /// The object a Library keeps at the top of that prefix once it has
    /// committed anything at all: the first link of its head chain
    /// (spec: FM-12, CP-1).
    const NAME: &str = "head-0.cfrt";

    /// A client whose one call is answered with `status` and an empty body.
    fn answering(status: u16) -> Client {
        let http_client = StaticReplayClient::new(vec![ReplayEvent::new(
            HttpRequest::empty(),
            HttpResponse::new(
                status.try_into().expect("the case names a real status"),
                SdkBody::empty(),
            ),
        )]);
        let config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("us-east-1"))
            .endpoint_url("http://storage.invalid")
            .credentials_provider(Credentials::new("key", "secret", None, None, "test"))
            .retry_config(RetryConfig::disabled())
            .force_path_style(true)
            .http_client(http_client)
            .build();

        Client::from_conf(config)
    }

    #[tokio::test]
    async fn an_object_s3_answers_for_is_there() {
        let result = check_object(&answering(200), BUCKET, PREFIX, NAME).await;
        assert!(
            matches!(result, Ok(true)),
            "expected the object to be there, got {result:?}"
        );
    }

    // The whole point of the `bool`: a prefix holding nothing is an answer and
    // not a failure, because a Library that has never been synced holds nothing
    // either and the two cannot be told apart from here.
    #[tokio::test]
    async fn an_object_that_is_not_there_is_an_answer_rather_than_a_refusal() {
        let result = check_object(&answering(404), BUCKET, PREFIX, NAME).await;
        assert!(
            matches!(result, Ok(false)),
            "expected an empty prefix to be an answer, got {result:?}"
        );
    }

    // Credentials S3 refused say nothing about whether the object is there, so
    // they must never come back as the `false` that a caller reads as an empty
    // Library.
    #[tokio::test]
    async fn credentials_s3_refused_are_not_an_empty_prefix() {
        let result = check_object(&answering(403), BUCKET, PREFIX, NAME).await;
        assert!(
            matches!(&result, Err(Error::PermissionDenied { .. })),
            "expected a refusal of the credentials, got {result:?}"
        );
    }

    // Nothing a person typed reaches what is reported: the prefix carries the
    // Library's own name on Storage, and the bucket is their configuration.
    #[tokio::test]
    async fn no_answer_carries_what_it_was_asked_about() {
        for status in [403, 401, 500] {
            let error = check_object(&answering(status), BUCKET, PREFIX, NAME)
                .await
                .expect_err("the case names a status S3 refuses with");
            assert!(!error.to_string().contains(BUCKET), "{status}: {error}");
            assert!(!error.to_string().contains(PREFIX), "{status}: {error}");
        }
    }
}
