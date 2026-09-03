//! Asking a bucket whether it is there, before there is a store over it.
//!
//! Everything else in this crate works inside one Library's prefix: [`S3`] is
//! built from the bucket and the prefix it is to act in, and the `ObjectStore`
//! port is scoped to a Library. This is the stage before that — the Library does
//! not exist here yet — which is why the call takes a client directly rather
//! than hanging off either the port or the store. Drive's counterpart is the
//! same shape and sits in the same place: `create_app_folder` is a pre-store
//! call, and it lives in the Drive gateway.
//!
//! It is a call worth making because on S3 a prefix exists only by being written
//! under, so nothing about setting a Library up would otherwise notice a
//! mistyped bucket, an endpoint nothing is listening at, or credentials the SDK
//! could not resolve: all three would look exactly like a Library that has never
//! been synced (spec: FM-18).
//!
//! It belongs to the gateway rather than to whoever creates a Library because
//! reading the answer means reading a status and an S3 error code, and this
//! crate already holds the one table that does that. A caller with a table of
//! its own would be a second answer able to disagree with the first — and the
//! only place the two could disagree is on what a person is told went wrong.
//!
//! [`S3`]: crate::S3

use aws_sdk_s3::Client;
use coffret_usecase::Result;

use crate::error::translate;

/// What the call is recorded and reported as.
const OPERATION: &str = "check_bucket";

/// What the answer names as missing, in place of the bucket itself.
///
/// The port's errors are rendered into the log, and a bucket is somebody's
/// configuration rather than anything this Library minted, so its name is not
/// written down there. Nothing is lost by describing the subject instead of
/// naming it: the caller asked the question and still holds the name it asked
/// with, which is what a person is told about.
const SUBJECT: &str = "the bucket";

/// Asks S3 whether `bucket` is there, and says why in the port's words if not.
///
/// The failure goes through the same table every other call in this crate does,
/// so the causes a caller has to tell apart arrive as separate variants rather
/// than as one message to be read: a bucket S3 answered about and does not hold
/// is [`Error::NotFound`] describing the bucket rather than naming it (see
/// `SUBJECT`), credentials that were resolved but not accepted are
/// [`Error::Unauthenticated`] or [`Error::PermissionDenied`], an endpoint
/// nothing is listening at is [`Error::Transport`], and credentials the SDK
/// could not resolve at all never become a request and are
/// [`Error::Unsupported`].
///
/// It asks nothing about any prefix, which is the point: a Library's own prefix
/// is empty until its first commit, so a question about that would answer the
/// same whether the bucket was reachable or not.
///
/// [`Error::NotFound`]: coffret_usecase::Error::NotFound
/// [`Error::Unauthenticated`]: coffret_usecase::Error::Unauthenticated
/// [`Error::PermissionDenied`]: coffret_usecase::Error::PermissionDenied
/// [`Error::Transport`]: coffret_usecase::Error::Transport
/// [`Error::Unsupported`]: coffret_usecase::Error::Unsupported
pub async fn check_bucket(client: &Client, bucket: &str) -> Result<()> {
    client
        .head_bucket()
        .bucket(bucket)
        .send()
        .await
        .map(|_| ())
        .map_err(|error| translate(OPERATION, SUBJECT, error))
}

#[cfg(test)]
mod tests {
    use aws_sdk_s3::config::retry::RetryConfig;
    use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
    use aws_smithy_http_client::test_util::{ReplayEvent, StaticReplayClient};
    use aws_smithy_runtime_api::client::orchestrator::{HttpRequest, HttpResponse};
    use aws_smithy_types::body::SdkBody;
    use coffret_usecase::Error;

    use super::*;

    /// The bucket the cases ask about.
    ///
    /// Nothing else in an answer reads like it, so a search for it finds it
    /// wherever it got in.
    const BUCKET: &str = "someones-holiday-photos";

    /// A client whose one call is answered with `status` and an empty body.
    ///
    /// `HeadBucket` is a `HEAD`, so an empty body is what a real answer carries
    /// too: the status is the whole of what S3 says, which is exactly the case
    /// the table has to classify without a message to read. Retries are off so
    /// that one call is one answer.
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
    async fn a_bucket_that_answers_is_there() {
        assert!(check_bucket(&answering(200), BUCKET).await.is_ok());
    }

    // A bucket S3 answered about and does not hold is `NotFound`, which is what
    // tells it apart from credentials that were refused and from an endpoint
    // nothing is listening at — and what it reports missing is `SUBJECT`.
    #[tokio::test]
    async fn a_bucket_s3_does_not_hold_is_not_found_without_being_named() {
        let result = check_bucket(&answering(404), BUCKET).await;
        let Err(Error::NotFound { object }) = &result else {
            panic!("expected a bucket S3 does not hold to be reported missing, got {result:?}");
        };
        assert_eq!(object, SUBJECT);
    }

    // Every answer, not only the one that reports the bucket missing: what the
    // port carries away from here is written into the log as it stands.
    #[tokio::test]
    async fn no_answer_carries_the_bucket_it_was_asked_about() {
        for status in [404, 403, 401, 500] {
            let error = check_bucket(&answering(status), BUCKET)
                .await
                .expect_err("the case names a status S3 refuses with");
            assert!(!error.to_string().contains(BUCKET), "{status}: {error}");
        }
    }

    // The case the variant name `BucketUnreachable` would have got wrong: the
    // bucket was reached, and what failed was the signature on the question.
    #[tokio::test]
    async fn credentials_s3_refused_are_told_apart_from_a_bucket_that_is_not_there() {
        let result = check_bucket(&answering(403), BUCKET).await;
        assert!(
            matches!(&result, Err(Error::PermissionDenied { .. })),
            "expected a refusal of the credentials, got {result:?}"
        );
    }
}
