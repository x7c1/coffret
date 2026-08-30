//! What every Drive test target here shares: the environment that turns a run
//! on, the log it writes, and a gateway built on a fresh subfolder.
//!
//! The variables, what they mean, and what a run leaves behind are documented
//! on the conformance target, which is the main consumer.

use std::sync::{Arc, Once};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use coffret_format::generate_library_id;
use coffret_logging::{install, LogSettings};
use coffret_model::MasterKey;
use google_drive_store::http::HttpTransport;
use google_drive_store::{
    create_app_folder, AccessTokens, ClientCredentials, DriveSettings, GoogleDrive, OAuthTokens,
    ReqwestTransport, TokenCache, DRIVE_FILE_SCOPE,
};
use tracing::Level;

/// The folder the test Libraries are created in.
///
/// Its presence is what turns the suite on.
pub const FOLDER_ID: &str = "COFFRET_DRIVE_FOLDER_ID";
/// The OAuth client to authorize as.
pub const CLIENT_ID: &str = "COFFRET_DRIVE_CLIENT_ID";
/// The OAuth client secret, for a client registered with one.
pub const CLIENT_SECRET: &str = "COFFRET_DRIVE_CLIENT_SECRET";
/// Where the grant was cached by the authorization flow.
pub const TOKEN_CACHE: &str = "COFFRET_DRIVE_TOKEN_CACHE";
/// The Master Key that cache is sealed under, base64 of 32 bytes.
pub const MASTER_KEY: &str = "COFFRET_MASTER_KEY";

/// The value of [`FOLDER_ID`] that means "the top of My Drive".
///
/// `root` is an alias for a folder this application did not create rather than
/// an id it may name, so asking for it as a parent is refused. Creating with no
/// parent at all is what Drive answers with the same placement, which is why
/// this value is passed to `create_app_folder` as no parent at all.
///
/// It costs tidiness: every case's folder lands at the top of My Drive, and
/// none of them is cleaned up. Prefer any real folder id.
pub const MY_DRIVE: &str = "root";

/// Points the run's events at the log file, once for the whole run.
///
/// This target is one of the few things in the workspace that is an
/// application: it talks to a real account, and what that account answers is
/// the evidence the suite exists to produce. Library crates emit and never
/// install a subscriber, so without this the run would emit into nothing.
///
/// `DEBUG`, because a run made by hand is exactly the occasion to keep every
/// call rather than only the surprises.
pub fn start_logging() {
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

/// The Master Key the token cache was sealed under.
///
/// The suite never writes a cache — the `authorize` example does — so this key
/// only has to be the one that example ran with.
pub fn master_key() -> MasterKey {
    let encoded = std::env::var(MASTER_KEY)
        .unwrap_or_else(|_| panic!("{FOLDER_ID} is set, so {MASTER_KEY} must be too"));

    let bytes = STANDARD
        .decode(encoded.trim())
        .unwrap_or_else(|error| panic!("{MASTER_KEY} must be base64: {error}"));

    MasterKey::from_bytes(bytes.as_slice().try_into().unwrap_or_else(|_| {
        panic!(
            "{MASTER_KEY} must decode to {} bytes, not {}",
            MasterKey::BYTE_LEN,
            bytes.len()
        )
    }))
}

/// Builds a gateway on a fresh app folder under the configured one, or `None`
/// when Drive is not configured.
///
/// `configure` adjusts the settings before the gateway is built — the
/// conformance suite shrinks the page size, an observation case takes the
/// defaults.
pub async fn drive(configure: impl FnOnce(DriveSettings) -> DriveSettings) -> Option<GoogleDrive> {
    let parent = std::env::var(FOLDER_ID).ok()?;

    // Only once the case is really going to run: a run that reports itself
    // skipped has nothing to record and leaves no file behind.
    start_logging();

    let client_id = std::env::var(CLIENT_ID)
        .unwrap_or_else(|_| panic!("{FOLDER_ID} is set, so {CLIENT_ID} must be too"));
    let cache = std::env::var(TOKEN_CACHE)
        .unwrap_or_else(|_| panic!("{FOLDER_ID} is set, so {TOKEN_CACHE} must be too"));

    let mut credentials = ClientCredentials::new(client_id);
    if let Ok(secret) = std::env::var(CLIENT_SECRET) {
        credentials = credentials.with_client_secret(secret);
    }

    let transport: Arc<dyn HttpTransport> = Arc::new(
        ReqwestTransport::with_default_client().expect("an HTTP client must be buildable"),
    );
    let tokens: Arc<dyn AccessTokens> = Arc::new(OAuthTokens::new(
        transport.clone(),
        credentials,
        TokenCache::new(cache, master_key()),
    ));

    // The gateway's own pre-store operation, so a run against a real account
    // exercises the code that creates a Library's folder rather than a second
    // implementation of it. Each case draws its own Library ID, which is what
    // gives it a folder nothing else in the run is working in (spec: FM-18).
    let library = generate_library_id().expect("the operating system must supply random bytes");
    let folder = create_app_folder(
        transport.clone(),
        tokens.clone(),
        (parent != MY_DRIVE).then_some(parent.as_str()),
        library,
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "{error}\n\
             Check that {FOLDER_ID} names a folder that still exists and that the \
             account authorized under {DRIVE_FILE_SCOPE} can write to; {MY_DRIVE:?} \
             is accepted as well, and puts the folder at the top of My Drive."
        )
    });
    let settings = configure(DriveSettings::new(folder));

    Some(GoogleDrive::new(transport, tokens, settings))
}
