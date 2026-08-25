//! Pointing a suite at a real S3 implementation.
//!
//! Five targets run a domain-crate suite against a live bucket — the
//! `ObjectStore` contract, the commit flow over it, the folder sync that produces
//! what a commit takes, the freeze that packs a folder into Packs instead, and
//! the fetch that carries the result back onto another device — and none of them
//! is about S3 wiring. This is that wiring, in one
//! place: which endpoint, which bucket, a key space nothing else in the run is
//! using, and where the run's events go.
//!
//! `make s3-store-it` starts MinIO in Docker, sets the environment below, runs
//! the targets, and tears the container down again. Without that environment
//! every case reports itself skipped, so an ordinary `cargo test` neither needs
//! Docker nor pretends to have covered S3.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::Client;
use coffret_logging::{install, LogSettings};
use s3_store::{S3Settings, S3};
use tracing::Level;

/// The endpoint of the S3 implementation to test against.
///
/// Its presence is what turns the suites on: everything else has a default.
const ENDPOINT: &str = "COFFRET_S3_IT_ENDPOINT";
/// The bucket to keep the test Libraries in.
const BUCKET: &str = "COFFRET_S3_IT_BUCKET";
/// The access key id to sign with.
const ACCESS_KEY: &str = "COFFRET_S3_IT_ACCESS_KEY";
/// The secret access key to sign with.
const SECRET_KEY: &str = "COFFRET_S3_IT_SECRET_KEY";

/// How many objects one listing page holds during a run.
///
/// Small, so a case reaches a second page by writing a handful of objects
/// instead of a thousand.
const PAGE_SIZE: i32 = 2;

/// A store over storage nothing else in this run is writing to, or `None` when
/// no endpoint is configured.
///
/// Every case starts from an empty store and the cases run in parallel in one
/// bucket, so each gets a key space of its own rather than a cleanup step that a
/// failing case would skip. `role` only makes the prefixes readable afterwards;
/// uniqueness comes from the clock and the counter.
///
/// The page size comes back with it because a suite that has to reach a second
/// listing page is the only thing that knows how many objects that takes, and
/// only this module knows what it was configured to be.
pub async fn store(role: &str) -> Option<(S3, usize)> {
    let (client, bucket) = client()?;

    // Only once a suite is really going to run: a run that reports itself
    // skipped has nothing to record and leaves no file behind.
    start_logging();
    ensure_bucket(&client, &bucket).await;

    let settings = S3Settings::new(bucket)
        .with_prefix(fresh_prefix(role))
        .with_page_size(PAGE_SIZE);

    Some((S3::new(client, settings), PAGE_SIZE as usize))
}

/// Builds a client for the configured endpoint, or `None` if there is none.
///
/// The region is fixed because MinIO ignores it and a signature needs one
/// anyway, and path-style addressing is forced because a bucket name is not a
/// subdomain of a container on localhost.
fn client() -> Option<(Client, String)> {
    let endpoint = std::env::var(ENDPOINT).ok()?;
    let bucket = std::env::var(BUCKET).unwrap_or_else(|_| "coffret-conformance".to_owned());
    let credentials = Credentials::new(
        std::env::var(ACCESS_KEY).unwrap_or_else(|_| "coffret-it".to_owned()),
        std::env::var(SECRET_KEY).unwrap_or_else(|_| "coffret-it-secret".to_owned()),
        None,
        None,
        "coffret-conformance",
    );
    let config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .endpoint_url(endpoint)
        .credentials_provider(credentials)
        .force_path_style(true)
        .build();

    Some((Client::from_conf(config), bucket))
}

/// A prefix nothing else in this run is using.
fn fresh_prefix(role: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock must be past the Unix epoch")
        .as_nanos();
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);

    format!("{role}/{nanos}-{sequence}")
}

/// Points the run's events at the log file, once for the whole run.
///
/// These targets are among the few things in the workspace that are an
/// application: they talk to a real S3 implementation, and what that
/// implementation answers is the evidence the suites exist to produce. Library
/// crates emit and never install a subscriber, so without this the run would
/// emit into nothing.
///
/// `DEBUG`, because a run made by hand is exactly the occasion to keep every
/// call rather than only the surprises.
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
