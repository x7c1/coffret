//! A Library to drive the routes against, and the way to ask them things.
//!
//! Two devices over one store, because that is what the interesting half of
//! these routes is about. One device holds the folder and syncs it into the
//! Library; the other is the one the server serves, and it starts with the
//! Library's Entries in its catalog and none of the files on disk — which is a
//! second enrolled device (spec: CK-9, EP-10), and the state in which `remote`
//! means something.
//!
//! Nothing here is a substitute for the real flows. The Entries are committed by
//! the sync itself and fetched by the fetch itself, over the use case's
//! in-memory store and catalog; what is stood in for is the provider and the
//! terminal, neither of which any of these routes touches.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use axum::Router;
use coffret_device::{EntryPath, OpenLibrary};
use coffret_local_fs::UnixFs;
use coffret_logging::testing::CapturedLogs;
use coffret_model::{LibraryId, MasterKey, MasterKeyEpoch};
use coffret_server::{
    catch_up_at_startup, router, Admission, Allowance, ServerState, UnlockPrompt, Unlocked,
    SERVER_KEY_HEADER,
};
use coffret_usecase::delete::{delete_entries, DeleteRequest, DeleteSelection};
use coffret_usecase::device_state::{BatchId, DeviceTime, Mapping, RootMarkerId};
// Aliased: `freeze_folder` is also the server's own way of arming a freeze,
// and the fixture uses both — this one to build a Library that already holds a
// Pack, the other (in `workers.rs`) to put a book on the worker.
use coffret_usecase::freeze::{freeze_folder as pack_directly, FreezeRequest};
use coffret_usecase::sync::{sync_folders, SyncRequest};
use coffret_usecase::{
    root_marker, InMemoryIndex, InMemoryStore, Index, LibraryKeys, ObjectStore, RefusingIndex,
};
use tempfile::TempDir;
use tokio::sync::mpsc;
use tracing::Level;

mod consent;
use consent::ScriptedConsent;
pub use consent::CONSENT_PAGE;

mod counting_store;
use counting_store::CountingStore;

mod drops;

mod halting_store;
use halting_store::HaltingStore;

mod local_folder;

mod readers;
pub use readers::{
    declined, deletion, files, fill, folders, folders_mapped, freeze, listing_of, rows_of, states,
    sync, without_server, written,
};

mod requests;

mod storage;

mod workers;

/// The address the server in these cases was started at.
///
/// A `Host` naming anywhere else is a request that reached this socket by
/// somebody else's name for it, so every case that means to be answered says
/// this one.
pub const AUTHORITY: &str = "127.0.0.1:8787";

/// The key the server in these cases drew as it started.
///
/// Fixed rather than drawn, so that a case can send the wrong one and say what
/// the right one was. Nothing about it is secret here: what it stands for is a
/// caller that read this device's own files, and the cases are the device.
pub const SERVER_KEY: &str = "8f14e45fceea167a5a36dedd4bea2543a1b2c3d4e5f60718293a4b5c6d7e8f90";

/// The Master Key the whole suite works under.
///
/// Real, because every case reads the Library back the way another device would:
/// a fixture that faked the keys would prove nothing about what that device
/// finds.
fn keys() -> LibraryKeys {
    LibraryKeys::derive(
        &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
        MasterKeyEpoch::FIRST,
    )
}

/// What the files of the Library are, and what is in each.
///
/// The names are the cases' vocabulary: two folders, a file beside a folder, and
/// one name a browser draws nothing from.
/// `café.jpg` is spelled with the accent as one code point, which is its NFC
/// spelling and the only one an Entry Path is in (spec: EP-1).
const PLANTED: [(&str, &[u8]); 6] = [
    ("albums/2026/spring.jpg", b"spring"),
    ("albums/2026/summer.jpg", b"summer"),
    ("albums/caf\u{e9}.jpg", b"a cafe"),
    ("albums/cover.png", b"cover"),
    ("albums/notes.txt", b"a note about the albums"),
    ("books/page-001.png", b"page one"),
];

/// One server over a Library another device filled.
pub struct Served {
    router: Router,
    /// The state the router answers out of, so a case can drive the background
    /// fill and wait for it rather than sleep on it.
    state: Arc<ServerState>,
    reads: Arc<CountingStore>,
    /// Storage, as a case can take away and give back.
    storage: Arc<HaltingStore>,
    /// The served device's catalog, as a case can make it stop saying what
    /// this device maps.
    catalog: Arc<RefusingIndex>,
    /// The other device's catalog, so a case can commit into the Library from
    /// somewhere other than the server under test.
    filled: InMemoryIndex,
    /// The store as the other device reaches it: the real one, behind neither
    /// the switch nor the counter, which are the served device's own.
    store: Arc<dyn ObjectStore>,
    /// The folder this device maps, so a case can put a file into it that the
    /// device did not place there.
    local: TempDir,
    /// The other device's folder, which is where a case plants what it is about
    /// to commit.
    remote: TempDir,
    /// Where both devices spool what they are about to upload, and kept so that
    /// nothing the fixture made is removed while a case runs.
    spools: TempDir,
    /// The disk they spool onto, which is the served device's own.
    local_fs: Arc<UnixFs>,
    /// How many batches the other device has committed, so each gets a name of
    /// its own (spec: OC-2).
    batches: AtomicUsize,
    /// The consent flow a reconnect starts, which a case ends when it says so.
    pub consent: Arc<ScriptedConsent>,
    /// The app's end of the unlock prompt, where the server was started the
    /// way the desktop app starts one, and `None` for the command line's.
    prompted: Option<std::sync::Mutex<mpsc::Receiver<()>>>,
}

impl Served {
    /// A server over a device that maps the whole Library.
    pub async fn library() -> Self {
        Self::mapping(None, false, Allowance::generous(), false)
            .await
            .started()
            .await
    }

    /// The same, started the way the desktop app starts one: with a prompt the
    /// server can wake to take the Passphrase again once it has locked.
    pub async fn in_the_app() -> Self {
        Self::mapping(None, false, Allowance::generous(), true)
            .await
            .started()
            .await
    }

    /// The same, serving within an allowance a case can actually reach: the
    /// case names the one budget it is about and takes the rest as they ship.
    pub async fn within(allowance: Allowance) -> Self {
        Self::mapping(None, false, allowance, false)
            .await
            .started()
            .await
    }

    /// The same, with the Library's `books` folder frozen into a Pack.
    ///
    /// What one case is about is a file that would replace an Entry inside a
    /// Pack, which is the one thing a drop is refused for that has nothing to do
    /// with its name (spec: PK-10, PK-12). It is a real freeze rather than a
    /// planted row, because what the refusal reads is the Container the catalog
    /// says the Entry lives in.
    pub async fn packed_library() -> Self {
        Self::mapping(None, true, Allowance::generous(), false)
            .await
            .started()
            .await
    }

    /// A server over a device that maps only one top-level component
    /// (spec: EP-9).
    ///
    /// Everything outside it is in the catalog and reaches no folder here, which
    /// is what an unmapped Entry is.
    pub async fn mapping_only(prefix: &str) -> Self {
        Self::mapping(
            Some(entry_path(prefix)),
            false,
            Allowance::generous(),
            false,
        )
        .await
        .started()
        .await
    }

    /// A server over a device that has joined the Library and never caught up.
    ///
    /// Its catalog stands at nothing, which is the state a device is in the
    /// moment it joins (spec: CK-9, RV-1) — and the state every other fixture
    /// here leaves by starting up. What the cases over this one are about is
    /// exactly that step: [`start_up`](Self::start_up) is the server's own first
    /// act, and until it has happened the Library is not on the screen at all.
    pub async fn joined() -> Self {
        Self::mapping(None, false, Allowance::generous(), false).await
    }

    async fn mapping(
        prefix: Option<EntryPath>,
        packed: bool,
        allowance: Allowance,
        prompting: bool,
    ) -> Self {
        let remote = tempfile::tempdir().expect("a temporary directory must be available");
        let local = tempfile::tempdir().expect("a temporary directory must be available");
        let spools = tempfile::tempdir().expect("a temporary directory must be available");
        let keys = keys();
        let store: Arc<dyn ObjectStore> = Arc::new(InMemoryStore::new(64));

        // The device that has the folder: it maps the Library root at it and
        // syncs, which is what puts the Entries in the Library at all.
        for (path, content) in PLANTED {
            plant(remote.path(), path, content);
        }
        let filled = InMemoryIndex::new();
        filled
            .set_mapping(Mapping::new(None, remote.path().to_path_buf()))
            .await
            .expect("a mapping is recorded");
        let local_fs = Arc::new(UnixFs::new());
        let outcome = sync_folders(SyncRequest::new(
            store.as_ref(),
            &filled,
            &keys,
            local_fs.as_ref(),
            local_fs.as_ref(),
            spools.path().join("filled"),
            BatchId::new("run-1"),
            DeviceTime::from_unix_seconds(1_700_000_000),
        ))
        .await
        .expect("the folder is carried into the Library");
        assert_eq!(
            outcome.added.len(),
            PLANTED.len(),
            "every planted file becomes an Entry: {outcome:?}",
        );

        // One folder of it repacked, where the case wants a Pack-resident Entry.
        // The freeze is the real one: what makes an Entry Pack-resident is the
        // Container the catalog names, and nothing else here would set it.
        if packed {
            pack_directly(FreezeRequest {
                prefix: Some(entry_path("books")),
                ..FreezeRequest::new(
                    store.as_ref(),
                    &filled,
                    &keys,
                    local_fs.as_ref(),
                    local_fs.as_ref(),
                    spools.path().join("filled"),
                    64 * 1024,
                    BatchId::new("run-2"),
                    DeviceTime::from_unix_seconds(1_700_000_100),
                )
            })
            .await
            .expect("one folder of the Library is packed");
        }

        // The device the server serves: the same Library, a folder of its own,
        // and nothing on disk yet. Its Storage is the same one, behind a switch
        // a case can turn off and a counter of what it read.
        let storage = Arc::new(HaltingStore::around(Arc::clone(&store)));
        let reads = Arc::new(CountingStore::around(
            Arc::clone(&storage) as Arc<dyn ObjectStore>
        ));
        let index = InMemoryIndex::new();
        index
            .set_mapping(
                Mapping::new(prefix.clone(), local.path().to_path_buf())
                    .expecting(register_root(local.path())),
            )
            .await
            .expect("a mapping is recorded");

        // Answering honestly, until a case says otherwise: the Library is
        // replayed into it at start-up like any other, and what a case that
        // takes it away meets is a catalog that went bad under a running
        // server.
        let catalog = Arc::new(RefusingIndex::around(index));

        let library = opened(&reads, &catalog, &local_fs, spools.path(), SERVED_LIBRARY);

        let consent = Arc::new(ScriptedConsent::default());
        let state = ServerState::new("served".to_owned(), library)
            .within(allowance)
            .consenting_through(Arc::clone(&consent) as _);
        let (state, prompted) = if prompting {
            let (prompt, asked) = UnlockPrompt::channel();
            (
                state.prompting_through(prompt),
                Some(std::sync::Mutex::new(asked)),
            )
        } else {
            (state, None)
        };
        let state = Arc::new(state);
        let admission = Arc::new(Admission::new(AUTHORITY, SERVER_KEY));
        Self {
            router: router(Arc::clone(&state), admission),
            state,
            reads,
            storage,
            catalog,
            filled,
            store,
            local,
            remote,
            spools,
            local_fs,
            batches: AtomicUsize::new(0),
            consent,
            prompted,
        }
    }

    /// The server's own first act: catching the catalog up with the Library.
    ///
    /// Every fixture but [`joined`](Self::joined) is handed over having done it,
    /// because that is what a running server has done — and because the rest of
    /// the cases are about a device that knows what the Library holds. What it
    /// cost is forgotten afterwards, so a case counting reads counts its own.
    async fn started(self) -> Self {
        self.start_up().await;
        self.reads.forget();
        self
    }

    /// Reopens the Library with the Passphrase and hands it to the locked
    /// server, the way the desktop app does once its window is answered.
    ///
    /// The same keys, Storage and catalog the server was started with — what a
    /// Passphrase reopens is the Library on this device, and that is these.
    pub fn unlock(&self) -> Unlocked {
        self.unlock_with(SERVED_LIBRARY)
            .expect("the Library this server serves is the one reopened")
    }

    /// The same, with a Library that says it is the one called `library_id`.
    pub fn unlock_with(&self, library_id: [u8; LibraryId::BYTE_LEN]) -> anyhow::Result<Unlocked> {
        self.state.unlock(opened(
            &self.reads,
            &self.catalog,
            &self.local_fs,
            self.spools.path(),
            library_id,
        ))
    }

    /// Whether the server woke the app's Passphrase window since this was last
    /// asked, which is never for a server started without one.
    pub fn prompt_woken(&self) -> bool {
        self.prompted.as_ref().is_some_and(|asked| {
            asked
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .try_recv()
                .is_ok()
        })
    }

    /// Catches the catalog up the way starting the server does.
    pub async fn start_up(&self) {
        catch_up_at_startup(&self.state).await;
    }

    /// Commits a file into the Library from the other device.
    ///
    /// The real sync over the real store, as the fixture's first commit is: what
    /// a case wants from this is a head the served device has not seen, and only
    /// a commit makes one.
    ///
    /// The store it goes through is the one underneath the switch a case can
    /// halt, and deliberately: what that switch stands for is *this* device's
    /// Storage going away, and a second device is not on the far end of it.
    pub async fn commit_elsewhere(&self, path: &str, content: &[u8]) {
        plant(self.remote.path(), path, content);
        let batch = self.batches.fetch_add(1, Ordering::SeqCst) + 1;
        let outcome = sync_folders(SyncRequest::new(
            self.store.as_ref(),
            &self.filled,
            &keys(),
            self.local_fs.as_ref(),
            self.local_fs.as_ref(),
            self.spools.path().join("filled"),
            // Named apart from the runs the fixture itself made, which the same
            // catalog holds the pending rows of (spec: OC-2).
            BatchId::new(format!("later-{batch}")),
            DeviceTime::from_unix_seconds(1_700_001_000 + batch as i64),
        ))
        .await
        .expect("the other device carries its folder into the Library");
        assert_eq!(
            outcome.added.len(),
            1,
            "one file was planted, so one Entry is committed: {outcome:?}",
        );
    }
}

impl Served {
    /// Packs everything the other device holds under `prefix` into one Pack,
    /// and commits it.
    ///
    /// The real freeze over the real store, from the other device: what a case
    /// wants from this is a Pack holding several Entries this device has never
    /// fetched, which is the shape a deletion rebuilds.
    pub async fn pack_elsewhere(&self, prefix: &str) {
        let batch = self.batches.fetch_add(1, Ordering::SeqCst) + 1;
        let outcome = pack_directly(FreezeRequest {
            prefix: Some(entry_path(prefix)),
            ..FreezeRequest::new(
                self.store.as_ref(),
                &self.filled,
                &keys(),
                self.local_fs.as_ref(),
                self.local_fs.as_ref(),
                self.spools.path().join("filled"),
                64 * 1024 * 1024,
                BatchId::new(format!("later-{batch}")),
                DeviceTime::from_unix_seconds(1_700_001_000 + batch as i64),
            )
        })
        .await
        .expect("the other device packs its folder");
        assert_eq!(outcome.packs.len(), 1, "one Pack: {outcome:?}");
    }

    /// Deletes one Entry from the Library on the other device, and commits it.
    pub async fn delete_elsewhere(&self, path: &str) {
        let batch = self.batches.fetch_add(1, Ordering::SeqCst) + 1;
        let outcome = delete_entries(DeleteRequest::new(
            self.store.as_ref(),
            &self.filled,
            &keys(),
            self.local_fs.as_ref(),
            self.spools.path().join("filled"),
            DeleteSelection::paths([entry_path(path)].into_iter().collect()),
            BatchId::new(format!("later-{batch}")),
            DeviceTime::from_unix_seconds(1_700_001_000 + batch as i64),
        ))
        .await
        .expect("the other device deletes the Entry");
        assert_eq!(
            outcome.entries(),
            1,
            "one Entry left the Library: {outcome:?}"
        );
    }
}

/// The Library the served device opens, by the identity it was created with.
const SERVED_LIBRARY: [u8; LibraryId::BYTE_LEN] = [0x11; LibraryId::BYTE_LEN];

/// The served device's Library as the Passphrase opens it: its Storage, its
/// catalog and its disk, under the suite's keys.
fn opened(
    reads: &Arc<CountingStore>,
    catalog: &Arc<RefusingIndex>,
    local_fs: &Arc<UnixFs>,
    spools: &Path,
    library_id: [u8; LibraryId::BYTE_LEN],
) -> OpenLibrary {
    OpenLibrary {
        store: Arc::clone(reads) as Arc<dyn ObjectStore>,
        index: Arc::clone(catalog) as Arc<dyn Index>,
        local_fs: Arc::clone(local_fs),
        keys: keys(),
        spool: spools.join("served"),
        library_id: LibraryId::from_bytes(library_id),
        epoch: MasterKeyEpoch::FIRST,
        provider: "s3",
        grant: None,
        births: Default::default(),
    }
}

/// The Entry Path a literal spells, or a panic naming the one that spells none.
///
/// Every Entry Path is built by reading text (spec: EP-1, EP-2), a fixture's
/// included; the unwrap lives here so that a mistyped literal is reported once,
/// as the mistake in the fixture that it is.
pub fn entry_path(text: impl Into<String>) -> EntryPath {
    EntryPath::parse(text)
        .unwrap_or_else(|error| panic!("a fixture holds a literal Entry Path: {error}"))
}

/// One request to a route, as the explorer on this device sends it.
///
/// The address the server was started at and the key it drew, on every request
/// rather than on the ones that thought to say so: a case here is about what a
/// route answers, and a case that had forgotten a header would be reporting the
/// admission fences as a broken route.
pub fn asking(method: &str, uri: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("host", AUTHORITY)
        .header(SERVER_KEY_HEADER, SERVER_KEY)
}

/// Gives the served device's mapped root the marker a placement compares against,
/// and hands back the identity its mapping records (spec: EP-13).
///
/// Every route that puts a file into that folder — a fill, one Entry fetched
/// because the browser opened it, a dropped file — descends through the same
/// check, so a root with no marker would refuse the lot. Written rather than
/// registered for real, because `set_mapping`'s registration wants a Library
/// directory on this device and these fixtures build their catalog by hand.
fn register_root(root: &Path) -> RootMarkerId {
    let id = RootMarkerId::from_bytes([0x2a; RootMarkerId::BYTE_LEN]);
    let area = root.join(root_marker::MANAGEMENT_AREA);
    std::fs::create_dir_all(&area).expect("a temporary folder is writable");
    std::fs::write(area.join(root_marker::MARKER_FILE), root_marker::spell(&id))
        .expect("a temporary file is writable");
    id
}

/// Writes one file under a folder, making the folders above it.
fn plant(root: &Path, path: &str, content: &[u8]) {
    let local = root.join(path);
    std::fs::create_dir_all(local.parent().expect("a planted file sits in a folder"))
        .expect("a temporary folder is writable");
    std::fs::write(&local, content).expect("a temporary file is writable");
}

/// The status and the JSON body of one answer.
pub async fn json(response: Response<Body>) -> (StatusCode, serde_json::Value) {
    let status = response.status();
    (
        status,
        serde_json::from_slice(&bytes(response).await).expect("the body is JSON"),
    )
}

/// The bytes of one answer.
pub async fn bytes(response: Response<Body>) -> Vec<u8> {
    axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the body is read whole")
        .to_vec()
}

/// What one header of an answer says.
pub fn header(response: &Response<Body>, name: &str) -> String {
    response
        .headers()
        .get(name)
        .unwrap_or_else(|| panic!("the answer carries a {name}"))
        .to_str()
        .expect("a header this server sets is ASCII")
        .to_owned()
}

/// One route asked, whichever verb it takes.
pub async fn route(served: &Served, method: &str, uri: &str) -> (StatusCode, serde_json::Value) {
    json(match method {
        "GET" => served.get(uri).await,
        _ => served.post(uri).await,
    })
    .await
}

/// The error event one refusal wrote, of the ones a case's own request made.
///
/// By `operation`, because a request drives more than the route: a fetch that
/// was declined may have caught the catalog up first, and a case asserting on
/// "the only error" would be asserting on whichever of them came last.
pub fn refusal_of(logs: &CapturedLogs, operation: &str) -> String {
    let events: Vec<String> = logs
        .at(Level::ERROR)
        .into_iter()
        .filter(|event| event.field("operation") == operation)
        .map(|event| event.field("error"))
        .collect();
    assert_eq!(
        events.len(),
        1,
        "expected one {operation} refusal, got {events:?}\nin:\n{}",
        logs.text(),
    );
    events.into_iter().next().expect("one refusal")
}
