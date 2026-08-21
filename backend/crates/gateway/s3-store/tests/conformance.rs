//! The `ObjectStore` conformance suite, run against a real S3 implementation.
//!
//! The suite itself lives with the port; what is here is only the wiring that
//! points it at a bucket. `make s3-store-it` starts MinIO in Docker, sets the
//! environment below, runs this target, and tears the container down again.
//! Without that environment the cases report themselves skipped, so an ordinary
//! `cargo test` neither needs Docker nor pretends to have covered S3.
//!
//! A configured run writes every call it makes to a file under
//! `$XDG_STATE_HOME/coffret/logs` — `$HOME/.local/state/coffret/logs` where that
//! is unset — and prints the name of it as it starts. This target is the only
//! thing in the workspace that drives this gateway, so without the sink here
//! everything the gateway records would be emitted into nothing.
//! `COFFRET_LOG_DIR` moves the file and `COFFRET_LOG_MAX_BYTES` bounds how much
//! is kept. It is JSONL — one JSON object per line, the fields each call was
//! recorded with kept as fields — so it is read with `jq` rather than an eye;
//! `make s3-store-it` carries the recipe.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::Client;
use coffret_logging::{install, LogSettings};
use coffret_usecase::conformance::StoreUnderTest;
use s3_store::{S3Settings, S3};
use tracing::Level;

/// The endpoint of the S3 implementation to test against.
///
/// Its presence is what turns the suite on: everything else has a default.
const ENDPOINT: &str = "COFFRET_S3_IT_ENDPOINT";
/// The bucket to keep the test Libraries in.
const BUCKET: &str = "COFFRET_S3_IT_BUCKET";
/// The access key id to sign with.
const ACCESS_KEY: &str = "COFFRET_S3_IT_ACCESS_KEY";
/// The secret access key to sign with.
const SECRET_KEY: &str = "COFFRET_S3_IT_SECRET_KEY";

/// How many objects one listing page holds during the suite.
///
/// Small, so the pagination case reaches a second page by writing a handful of
/// objects instead of a thousand.
const PAGE_SIZE: i32 = 2;

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
///
/// Every case starts from an empty store, and the cases run in parallel in one
/// bucket, so each gets a key space of its own rather than a cleanup step that
/// a failing case would skip.
fn fresh_prefix() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock must be past the Unix epoch")
        .as_nanos();
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);

    format!("conformance/{nanos}-{sequence}")
}

/// Points the run's events at the log file, once for the whole run.
///
/// This target is one of the few things in the workspace that is an
/// application: it talks to a real S3 implementation, and what that
/// implementation answers is the evidence the suite exists to produce. Library
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

/// Hands the suite an empty store, or `None` when no endpoint is configured.
async fn fixture() -> Option<StoreUnderTest> {
    let (client, bucket) = client()?;

    // Only once the suite is really going to run: a run that reports itself
    // skipped has nothing to record and leaves no file behind.
    start_logging();

    ensure_bucket(&client, &bucket).await;

    let settings = S3Settings::new(bucket)
        .with_prefix(fresh_prefix())
        .with_page_size(PAGE_SIZE);

    Some(StoreUnderTest::new(
        Box::new(S3::new(client, settings)),
        PAGE_SIZE as usize,
    ))
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

coffret_usecase::object_store_conformance!(fixture().await);
