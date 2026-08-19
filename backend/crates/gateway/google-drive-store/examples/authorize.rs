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
//!
//! cargo run -p google-drive-store --example authorize
//! ```
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

use google_drive_store::{
    Authorization, ClientCredentials, HttpTransport, ReqwestTransport, TokenCache, DRIVE_FILE_SCOPE,
};

#[tokio::main]
async fn main() {
    let client_id = require("COFFRET_DRIVE_CLIENT_ID");
    let cache = TokenCache::new(require("COFFRET_DRIVE_TOKEN_CACHE"));

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
        Ok(()) => println!("Authorized. The grant is cached at {:?}.", cache.path()),
        Err(error) => {
            eprintln!("Authorization failed: {error}");
            std::process::exit(1);
        }
    }
}

/// Reads a variable the flow cannot run without.
fn require(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        eprintln!("{name} must be set");
        std::process::exit(1);
    })
}
