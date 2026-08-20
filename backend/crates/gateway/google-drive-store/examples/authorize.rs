//! Runs the Google authorization flow once and caches the grant.
//!
//! Authorizing needs a person at a browser, so it cannot be part of any test
//! run. This is the entry point for doing it by hand: it prints a URL, waits on
//! a loopback port for the redirect, and writes the refresh token to the cache
//! that everything else reads.
//!
//! ```text
//! COFFRET_DRIVE_CLIENT_ID      the OAuth client to authorize as
//! COFFRET_DRIVE_CLIENT_SECRET  optional, for a client registered with one
//! COFFRET_DRIVE_TOKEN_CACHE    where to cache the grant
//! COFFRET_MASTER_KEY           the Master Key the cache is sealed under,
//!                              base64 of 32 bytes
//!
//! cargo run -p google-drive-store --example authorize
//! ```
//!
//! What the endpoint answered is logged to a file under the state directory —
//! `$XDG_STATE_HOME/coffret/logs`, or `$HOME/.local/state/coffret/logs` — since
//! a flow that fails does so against Google's answer, and that answer is the
//! only thing that says why. No token is written there.
//!
//! The grant it asks for is `drive.file` alone, so the consent screen offers
//! access to the files this application creates and nothing else in the
//! account.
//!
//! The OAuth client has to be registered as a desktop client. The redirect this
//! flow listens on is a loopback address whose port the operating system hands
//! out, and only a desktop client may redirect to one; a web client's redirect
//! URIs are registered one exact URL at a time, so authorizing as one is
//! refused with `redirect_uri_mismatch` before the consent screen appears.

use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use coffret_logging::{install, LogSettings};
use coffret_model::MasterKey;
use google_drive_store::{
    Authorization, ClientCredentials, HttpTransport, ReqwestTransport, TokenCache, DRIVE_FILE_SCOPE,
};

#[tokio::main]
async fn main() {
    start_logging();

    let client_id = require("COFFRET_DRIVE_CLIENT_ID");
    let cache = TokenCache::new(require("COFFRET_DRIVE_TOKEN_CACHE"), master_key());

    let mut credentials = ClientCredentials::new(client_id);
    if let Ok(secret) = std::env::var("COFFRET_DRIVE_CLIENT_SECRET") {
        credentials = credentials.with_client_secret(secret);
    }

    let transport: Arc<dyn HttpTransport> = Arc::new(
        ReqwestTransport::with_default_client().expect("an HTTP client must be buildable"),
    );

    println!("Asking for {DRIVE_FILE_SCOPE} and nothing else.");
    let outcome = Authorization::new(transport, credentials, cache.clone())
        .run(|url| println!("\nOpen this in a browser:\n\n{url}\n"))
        .await;

    match outcome {
        Ok(()) => println!(
            "Authorized. The grant is cached, sealed, at {:?}.",
            cache.path()
        ),
        Err(error) => {
            eprintln!("Authorization failed: {error}");
            std::process::exit(1);
        }
    }
}

/// Points this run's events at the log file.
///
/// An example is an application, and an application is what installs a
/// subscriber: the library crates it drives only emit.
fn start_logging() {
    let settings = LogSettings::from_env().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1);
    });
    match install(&settings) {
        Ok(path) => println!("Logging this run to {}.", path.display()),
        Err(error) => {
            eprintln!("Could not start logging: {error}");
            std::process::exit(1);
        }
    }
}

/// Reads the Master Key the cache is sealed under.
///
/// The cache is encrypted under a key derived from this one, so whatever reads
/// it afterwards has to be given the same Master Key. `openssl rand -base64 32`
/// mints one; taking it from the environment is what stands in until a device
/// unlocks its stored Master Key from a Passphrase.
fn master_key() -> MasterKey {
    let encoded = require("COFFRET_MASTER_KEY");
    let bytes = STANDARD.decode(encoded.trim()).unwrap_or_else(|error| {
        eprintln!("COFFRET_MASTER_KEY must be base64: {error}");
        std::process::exit(1);
    });

    let bytes: [u8; MasterKey::BYTE_LEN] = bytes.as_slice().try_into().unwrap_or_else(|_| {
        eprintln!(
            "COFFRET_MASTER_KEY must decode to {} bytes, not {}",
            MasterKey::BYTE_LEN,
            bytes.len()
        );
        std::process::exit(1);
    });
    MasterKey::from_bytes(bytes)
}

/// Reads a variable the flow cannot run without.
fn require(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        eprintln!("{name} must be set");
        std::process::exit(1);
    })
}
