//! Pointing this crate's cases at a real S3 implementation.
//!
//! Creating an S3 Library needs no provider at all, so the crate's own unit
//! tests cover that. What needs one is opening a Library: the settings file
//! says where the bucket is and nothing about how to reach it, and whether that
//! is enough is a question only a real endpoint answers.
//!
//! `make s3-store-it` starts MinIO in Docker, sets the environment below, runs
//! the targets, and tears the container down again. Without that environment
//! every case here reports itself skipped, so an ordinary `cargo test` neither
//! needs Docker nor pretends to have covered S3.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::Client;
use coffret_logging::{install, LogSettings};
use tracing::Level;

/// The endpoint of the S3 implementation to test against.
///
/// Its presence is what turns the cases on: everything else has a default.
const ENDPOINT: &str = "COFFRET_S3_IT_ENDPOINT";
/// The bucket to keep the test Libraries in.
const BUCKET: &str = "COFFRET_S3_IT_BUCKET";
/// The access key id to sign with.
const ACCESS_KEY: &str = "COFFRET_S3_IT_ACCESS_KEY";
/// The secret access key to sign with.
const SECRET_KEY: &str = "COFFRET_S3_IT_SECRET_KEY";

/// The region every case signs for.
///
/// Fixed because MinIO ignores it and a signature needs one anyway.
pub const REGION: &str = "us-east-1";

/// Where a Library on the S3 implementation lives, for one case.
pub struct Target {
    /// The bucket to create the Library in.
    pub bucket: String,
    /// The endpoint to talk to.
    pub endpoint: String,
    /// A base prefix nothing else in this run is using.
    pub base_prefix: String,
}

/// The bucket and endpoint to run one case against, or `None` where there is no
/// S3 implementation configured.
///
/// The bucket is created if this is the first case to need it, the run's log
/// file is started, and a base prefix nothing else is using comes back with it:
/// the cases run in parallel against one bucket, so each gets a key space of its
/// own rather than a cleanup step a failing case would skip.
pub async fn target(role: &str) -> Option<Target> {
    let endpoint = std::env::var(ENDPOINT).ok()?;
    let bucket = std::env::var(BUCKET).unwrap_or_else(|_| "coffret-conformance".to_owned());

    // Only once a case is really going to run: a run that reports itself
    // skipped has nothing to record and leaves no file behind.
    start_logging();
    require_sdk_credentials();
    use_own_state_dir();
    ensure_bucket(&client(&endpoint), &bucket).await;

    Some(Target {
        bucket,
        endpoint,
        base_prefix: fresh_prefix(role),
    })
}

/// Points every case in this binary at a state directory of its own.
///
/// One for the whole binary rather than one per case, because the directory is
/// named by an environment variable and a variable is one value for a process.
/// Cases are told apart by Library name instead.
fn use_own_state_dir() {
    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        std::env::set_var(coffret_device::STATE_DIRECTORY, directory.keep());
    });
}

/// A client for the configured endpoint, for the setup this crate does itself.
///
/// The credentials are named here because creating the bucket is the harness's
/// own work. What is being tested resolves its own — a Library's settings say
/// where the bucket is and never how to sign for it — which is what
/// [`require_sdk_credentials`] is about.
fn client(endpoint: &str) -> Client {
    let credentials = Credentials::new(
        std::env::var(ACCESS_KEY).unwrap_or_else(|_| "coffret-it".to_owned()),
        std::env::var(SECRET_KEY).unwrap_or_else(|_| "coffret-it-secret".to_owned()),
        None,
        None,
        "coffret-device-integration",
    );
    let config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new(REGION))
        .endpoint_url(endpoint)
        .credentials_provider(credentials)
        .force_path_style(true)
        .build();

    Client::from_conf(config)
}

/// Refuses to run where the SDK's own resolution would find nothing.
///
/// The point of the case is that opening a Library takes its credentials from
/// the environment the SDK reads rather than from anything coffret wrote, so a
/// run where that environment is empty would fail for a reason that says
/// nothing about the code under test.
fn require_sdk_credentials() {
    assert!(
        std::env::var_os("AWS_ACCESS_KEY_ID").is_some(),
        "AWS_ACCESS_KEY_ID must be set for the SDK to resolve; run this through `make s3-store-it`"
    );
}

/// A base prefix nothing else in this run is using.
fn fresh_prefix(role: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock must be past the Unix epoch")
        .as_nanos();
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);

    format!("{role}/{nanos}-{sequence}/")
}

/// Points the run's events at the log file, once for the whole run.
fn start_logging() {
    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        let settings = LogSettings::from_env()
            .expect("the log settings must be readable")
            .with_level(Level::DEBUG);

        match install(&settings) {
            // Printed rather than logged: the point of it is to be read by
            // whoever started the run, who is standing at a terminal.
            Ok(path) => eprintln!("logging this run to {}", path.display()),
            Err(error) => panic!("could not start logging: {error}"),
        }
    });
}

/// Creates the bucket if this is the first case to need it.
async fn ensure_bucket(client: &Client, bucket: &str) {
    match client.create_bucket().bucket(bucket).send().await {
        Ok(_) => {}
        // Whichever case ran first created it; the rest race and lose, which is
        // the expected outcome rather than a failure.
        Err(SdkError::ServiceError(service))
            if matches!(service.raw().status().as_u16(), 409 | 200) => {}
        Err(error) => panic!("could not create the test bucket {bucket:?}: {error}"),
    }
}
