//! The S3 implementation a case runs against, and the harness's own client for
//! it.

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::Client;

use super::REGION;

/// Where a Library on the S3 implementation lives, for one case.
pub struct Minio {
    /// The bucket to keep the Library in.
    pub bucket: String,
    /// The endpoint to talk to.
    pub endpoint: String,
    /// A base prefix nothing else in this run is using.
    pub base_prefix: String,
}

/// The endpoint of the S3 implementation to test against.
const IT_ENDPOINT: &str = "COFFRET_S3_IT_ENDPOINT";
/// The bucket to keep the test Libraries in.
const IT_BUCKET: &str = "COFFRET_S3_IT_BUCKET";

/// The bucket and endpoint to run one case against, or `None` where there is no
/// S3 implementation configured.
///
/// The cases run in parallel against one bucket, so each gets a base prefix
/// nothing else is using rather than a cleanup step a failing case would skip.
pub fn minio(role: &str) -> Option<Minio> {
    let endpoint = std::env::var(IT_ENDPOINT).ok()?;
    signing_credentials();

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock must be past the Unix epoch")
        .as_nanos();
    Some(Minio {
        bucket: std::env::var(IT_BUCKET).unwrap_or_else(|_| "coffret-conformance".to_owned()),
        endpoint,
        base_prefix: format!("cli/{role}/{nanos}/"),
    })
}

impl Minio {
    /// The harness's own client for this endpoint.
    ///
    /// Its own, and deliberately not the one under test: what a case asks with
    /// this is what is really in the bucket, which is the one question the
    /// binary cannot be trusted to answer about itself.
    pub fn client(&self) -> Client {
        let credentials = Credentials::new(
            std::env::var("AWS_ACCESS_KEY_ID").expect("the harness signs with what it exported"),
            std::env::var("AWS_SECRET_ACCESS_KEY")
                .expect("the harness signs with what it exported"),
            None,
            None,
            "coffret-cli-integration",
        );
        let config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(REGION))
            .endpoint_url(&self.endpoint)
            .credentials_provider(credentials)
            .force_path_style(true)
            .build();

        Client::from_conf(config)
    }

    /// Creates the bucket if this is the first case to need it.
    pub async fn ensure_bucket(&self) {
        match self
            .client()
            .create_bucket()
            .bucket(&self.bucket)
            .send()
            .await
        {
            Ok(_) => {}
            // Whichever case ran first created it; the rest race and lose,
            // which is the expected outcome rather than a failure.
            Err(SdkError::ServiceError(service))
                if matches!(service.raw().status().as_u16(), 409 | 200) => {}
            Err(error) => panic!(
                "could not create the test bucket {:?}: {error}",
                self.bucket
            ),
        }
    }

    /// Every key under `prefix`, in the order S3 lists them.
    pub async fn keys_under(&self, prefix: &str) -> Vec<String> {
        let mut keys = Vec::new();
        let mut token = None;
        loop {
            let page = self
                .client()
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(prefix)
                .set_continuation_token(token)
                .send()
                .await
                .expect("the bucket must be listable");

            keys.extend(
                page.contents()
                    .iter()
                    .filter_map(|object| object.key().map(str::to_owned)),
            );
            token = page.next_continuation_token().map(str::to_owned);
            if token.is_none() {
                return keys;
            }
        }
    }
}

/// Puts credentials where the SDK's own resolution will find them.
///
/// The binary under test resolves its own — a Library's settings say where its
/// bucket is and never how to sign for it — so on a machine with none
/// configured the resolution itself is what would fail a case. What is signed
/// with is never checked by [`stub_endpoint`](super::stub_endpoint), and under
/// `make s3-store-it` the harness has already exported MinIO's, which these
/// leave alone.
pub(super) fn signing_credentials() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        for (name, value) in [
            ("AWS_ACCESS_KEY_ID", "coffret-cli-tests"),
            ("AWS_SECRET_ACCESS_KEY", "coffret-cli-tests-secret"),
            ("AWS_REGION", REGION),
        ] {
            if std::env::var_os(name).is_none() {
                std::env::set_var(name, value);
            }
        }
    });
}
