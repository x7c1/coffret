//! What every Drive test target here shares: the environment that turns a run
//! on, the log it writes, and a gateway built on a fresh subfolder.
//!
//! The variables, what they mean, and what a run leaves behind are documented
//! on the conformance target, which is the main consumer.

use std::ops::Range;
use std::sync::{Arc, Once};

use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use coffret_format::{generate_library_id, Purpose, PurposeKey};
use coffret_logging::{install, LogSettings};
use coffret_model::{MasterKey, ObjectRef};
use coffret_usecase::{ByteStream, CommitSlot, ObjectPage, ObjectStore, PageToken, Result};
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

/// The value of [`FOLDER_ID`] no run may use.
///
/// `root` is an alias for a folder this application did not create rather than
/// an id it may name, so Drive refuses it as a parent; the placement it stands
/// for — the top of My Drive — is not one a run may ask for either, because
/// that is where a person's own folders are and not where a test's belong.
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

/// The key the token cache was sealed under, derived from the configured
/// Master Key for that one purpose (spec: KD-4).
///
/// The suite never writes a cache — the `authorize` example does — so this key
/// only has to be the one that example ran with.
pub fn token_cache_key() -> PurposeKey {
    PurposeKey::derive(&master_key(), Purpose::TokenCache)
}

/// The Master Key the token cache was sealed under.
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

/// Builds a gateway on a fresh app folder under the configured one, or `None`
/// when Drive is not configured.
///
/// `configure` adjusts the settings before the gateway is built — the
/// conformance suite shrinks the page size, an observation case takes the
/// defaults.
///
/// What comes back trashes its folder when the case ends, so a run against a
/// real account leaves it as it found it.
pub async fn drive(configure: impl FnOnce(DriveSettings) -> DriveSettings) -> Option<CaseFolder> {
    let parent = std::env::var(FOLDER_ID).ok()?;
    assert_ne!(
        parent, MY_DRIVE,
        "{FOLDER_ID} must name a folder of your own; {MY_DRIVE:?} is not an id a folder may be \
         created under"
    );

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
        TokenCache::new(cache, Arc::new(token_cache_key())),
    ));

    // The gateway's own pre-store operation, so a run against a real account
    // exercises the code that creates a Library's folder rather than a second
    // implementation of it. Each case draws its own Library ID, which is what
    // gives it a folder nothing else in the run is working in (spec: FM-18).
    let library = generate_library_id().expect("the operating system must supply random bytes");
    let folder = create_app_folder(transport.clone(), tokens.clone(), &parent, library)
        .await
        .unwrap_or_else(|error| {
            panic!(
                "{error}\n\
                 Check that {FOLDER_ID} names a folder that still exists and that the \
                 account authorized under {DRIVE_FILE_SCOPE} can write to."
            )
        });
    let settings = configure(DriveSettings::new(&folder));

    Some(CaseFolder {
        drive: GoogleDrive::new(transport, tokens, settings),
        folder: ObjectRef::new(folder),
    })
}

/// The store one case works in, and the folder it goes away with.
///
/// A case creates a folder of its own so that the cases neither see each other's
/// objects nor need the configured folder to be empty, and a run that left every
/// one of them behind would fill a real account with `coffret-` folders nobody
/// asked for. So the folder is trashed when the case ends — trashed and not
/// purged, because what a failing case left is evidence, and Drive's trash is
/// where evidence goes to be recoverable for a while rather than gone.
///
/// Everything else is the gateway's, delegated: what the suite exercises must be
/// the store under test and not a wrapper's idea of it.
pub struct CaseFolder {
    drive: GoogleDrive,
    folder: ObjectRef,
}

#[async_trait]
impl ObjectStore for CaseFolder {
    async fn put(&self, name: &str, body: ByteStream) -> Result<ObjectRef> {
        self.drive.put(name, body).await
    }

    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        self.drive.reserve_create(name).await
    }

    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        self.drive.put_if_absent(slot, body).await
    }

    fn object_at(&self, slot: &CommitSlot) -> Result<ObjectRef> {
        self.drive.object_at(slot)
    }

    async fn get(&self, object: &ObjectRef, range: Option<Range<u64>>) -> Result<ByteStream> {
        self.drive.get(object, range).await
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        self.drive.list(page).await
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        self.drive.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.drive.purge(object).await
    }
}

impl Drop for CaseFolder {
    fn drop(&mut self) {
        // A `Drop` cannot await and the case's own runtime is on its way out by
        // the time this runs, so the removal gets a thread and a runtime of its
        // own, and the drop waits for it. A folder left behind would otherwise
        // outlive the run silently, which is the one outcome this exists to
        // prevent.
        let removal = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("a runtime for the removal must be buildable")
                        .block_on(self.drive.trash(&self.folder))
                })
                .join()
        });

        // Printed rather than panicked over: a `Drop` that panics while the case
        // is already failing replaces the failure with itself, and what is left
        // behind is a folder in the trash rather than a wrong answer.
        match removal {
            Ok(Err(error)) => eprintln!("could not trash the case folder {}: {error}", self.folder),
            Err(_) => eprintln!("the removal of the case folder {} panicked", self.folder),
            Ok(Ok(())) => {}
        }
    }
}
