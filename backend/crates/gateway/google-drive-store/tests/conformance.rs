//! The `ObjectStore` conformance suite, run against a real Google Drive folder.
//!
//! The same suite the S3 gateway runs in CI, pointed at Drive. It is not part of
//! any automated run and never will be: it needs a Google account, a grant a
//! person clicked through, and calls against a live API. It exists so that the
//! two adapters are held to one contract rather than to whatever each provider
//! happened to make easy, and it is run by hand when the Drive adapter changes.
//!
//! Authorize once with the `authorize` example, then set:
//!
//! ```text
//! COFFRET_DRIVE_FOLDER_ID      the folder to work in; its presence turns the suite on
//! COFFRET_DRIVE_CLIENT_ID      the OAuth client to authorize as
//! COFFRET_DRIVE_CLIENT_SECRET  optional, for a client registered with one
//! COFFRET_DRIVE_TOKEN_CACHE    where the grant was cached
//! COFFRET_MASTER_KEY           the Master Key that cache was sealed under
//! ```
//!
//! The cache is encrypted, so `COFFRET_MASTER_KEY` has to be the same value the
//! `authorize` example ran with; under any other one the cache does not open and
//! the suite fails before its first call.
//!
//! `COFFRET_DRIVE_FOLDER_ID` may be any folder — one made in the Drive web
//! interface included. A `drive.file` grant reaches only what this application
//! created, but that restricts what may be *read*, not where something new may
//! be put: naming a folder as the parent of a file being created is allowed,
//! and each case only creates a subfolder of its own and stays inside it, so it
//! never asks to read the folder it was given. `root` is the exception, and
//! `MY_DRIVE` says why.
//!
//! Each case works in a subfolder of its own, so the cases neither see each
//! other's objects nor need the configured folder to be empty. The subfolders
//! are left behind: they are the record of a run, and deleting them from a case
//! that failed would delete the evidence.
//!
//! So is the log. A configured run writes every call it makes to a file under
//! `$XDG_STATE_HOME/coffret/logs` — `$HOME/.local/state/coffret/logs` where
//! that is unset — and prints the name of it as it starts. That file is what
//! answers "what does Drive actually send when this happens?" afterwards, which
//! is the question this suite exists to have an answer to. `COFFRET_LOG_DIR`
//! moves it and `COFFRET_LOG_MAX_BYTES` bounds how much is kept.

use std::sync::{Arc, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use coffret_logging::{install, LogSettings};
use coffret_model::MasterKey;
use coffret_usecase::conformance::StoreUnderTest;
use google_drive_store::http::{HttpRequest, HttpTransport, Method};
use google_drive_store::{
    AccessTokens, ClientCredentials, DriveSettings, GoogleDrive, OAuthTokens, ReqwestTransport,
    TokenCache, DRIVE_API, DRIVE_FILE_SCOPE,
};
use tracing::Level;

/// The folder the test Libraries are created in.
///
/// Its presence is what turns the suite on.
const FOLDER_ID: &str = "COFFRET_DRIVE_FOLDER_ID";
/// The OAuth client to authorize as.
const CLIENT_ID: &str = "COFFRET_DRIVE_CLIENT_ID";
/// The OAuth client secret, for a client registered with one.
const CLIENT_SECRET: &str = "COFFRET_DRIVE_CLIENT_SECRET";
/// Where the grant was cached by the authorization flow.
const TOKEN_CACHE: &str = "COFFRET_DRIVE_TOKEN_CACHE";
/// The Master Key that cache is sealed under, base64 of 32 bytes.
const MASTER_KEY: &str = "COFFRET_MASTER_KEY";

/// The value of [`FOLDER_ID`] that means "the top of My Drive".
///
/// `root` is an alias for a folder this application did not create rather than
/// an id it may name, so asking for it as a parent is refused. Creating with no
/// parent at all is what Drive answers with the same placement, which is why
/// [`create_folder`] omits the field for this value instead of passing it on.
///
/// It costs tidiness: every case's folder lands at the top of My Drive, and
/// none of them is cleaned up. Prefer any real folder id.
const MY_DRIVE: &str = "root";

/// How many objects one listing page holds during the suite.
///
/// Small, so the pagination case reaches a second page by writing a handful of
/// objects instead of a thousand.
const PAGE_SIZE: i32 = 2;

/// Points the run's events at the log file, once for the whole run.
///
/// This target is one of the few things in the workspace that is an
/// application: it talks to a real account, and what that account answers is
/// the evidence the suite exists to produce. Library crates emit and never
/// install a subscriber, so without this the run would emit into nothing.
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

/// Hands the suite an empty store, or `None` when Drive is not configured.
async fn fixture() -> Option<StoreUnderTest> {
    let parent = std::env::var(FOLDER_ID).ok()?;

    // Only once the suite is really going to run: a run that reports itself
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

    let folder = create_folder(transport.as_ref(), tokens.as_ref(), &parent, &fresh_name()).await;
    let settings = DriveSettings::new(folder).with_page_size(PAGE_SIZE);

    Some(StoreUnderTest::new(
        Box::new(GoogleDrive::new(transport, tokens, settings)),
        PAGE_SIZE as usize,
    ))
}

/// The Master Key the token cache was sealed under.
///
/// The suite never writes a cache — the `authorize` example does — so this key
/// only has to be the one that example ran with.
fn master_key() -> MasterKey {
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

/// A folder name nothing else in this run is using.
fn fresh_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock must be past the Unix epoch")
        .as_nanos();

    format!("coffret-conformance-{nanos}")
}

/// Creates a subfolder and reports its id.
///
/// The gateway has no notion of folders — a Library reaches Storage as a flat
/// set of Storage Objects — so this speaks to Drive directly, through the same
/// transport the gateway uses.
async fn create_folder(
    transport: &dyn HttpTransport,
    tokens: &dyn AccessTokens,
    parent: &str,
    name: &str,
) -> String {
    let token = tokens
        .access_token()
        .await
        .expect("the cached grant must still mint tokens; re-run the authorize example");

    let mut metadata = serde_json::json!({
        "name": name,
        "mimeType": "application/vnd.google-apps.folder",
    });
    // Naming no parent is what puts the folder at the top of My Drive; naming
    // `root` as one would ask for access to a folder the grant does not cover.
    if parent != MY_DRIVE {
        metadata["parents"] = serde_json::json!([parent]);
    }
    let request = HttpRequest::new(Method::Post, format!("{DRIVE_API}/files?fields=id"))
        .with_header("authorization", format!("Bearer {token}"))
        .with_json(&metadata);

    let response = transport
        .execute(request)
        .await
        .expect("creating the case's folder must reach Drive");
    let status = response.status();
    let body = response
        .into_body()
        .into_bytes()
        .await
        .expect("Drive's answer must be readable");

    assert!(
        (200..300).contains(&status),
        "could not create a folder under {parent:?}: {status} {}\n\
         Check that {FOLDER_ID} names a folder that still exists and that the \
         account authorized under {DRIVE_FILE_SCOPE} can write to; {MY_DRIVE:?} \
         is accepted as well, and puts the folder at the top of My Drive.",
        String::from_utf8_lossy(&body)
    );

    let created: serde_json::Value =
        serde_json::from_slice(&body).expect("Drive must answer with JSON");

    created["id"]
        .as_str()
        .expect("Drive must report the folder's id")
        .to_owned()
}

coffret_usecase::object_store_conformance!(fixture().await);
