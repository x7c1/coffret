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
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use axum::Router;
use coffret_device::{EntryPath, OpenLibrary};
use coffret_model::{LibraryId, MasterKey, MasterKeyEpoch};
use coffret_server::{
    catch_up_at_startup, fill_folder, freeze_folder, lock_when_idle, router, Admission, Folder,
    ServerState, CAPABILITY_HEADER,
};
use coffret_usecase::device_state::{BatchId, DeviceTime, Mapping};
// Aliased: `freeze_folder` is also the server's own way of arming a freeze,
// and the fixture uses both — this one to build a Library that already holds a
// Pack, the other to put a book on the worker.
use coffret_usecase::freeze::{freeze_folder as pack_directly, FreezeRequest};
use coffret_usecase::sync::{sync_folders, SyncRequest};
use coffret_usecase::{InMemoryIndex, InMemoryStore, Index, LibraryKeys, ObjectStore};
use tempfile::TempDir;
use tokio::task::JoinHandle;
use tower::ServiceExt;

mod counting_store;
use counting_store::CountingStore;

mod halting_store;
use halting_store::HaltingStore;

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
    /// How many batches the other device has committed, so each gets a name of
    /// its own (spec: OC-2).
    batches: AtomicUsize,
}

impl Served {
    /// A server over a device that maps the whole Library.
    pub async fn library() -> Self {
        Self::mapping(None, false).await.started().await
    }

    /// The same, with the Library's `books` folder frozen into a Pack.
    ///
    /// What one case is about is a file that would replace an Entry inside a
    /// Pack, which is the one thing a drop is refused for that has nothing to do
    /// with its name (spec: PK-10, PK-12). It is a real freeze rather than a
    /// planted row, because what the refusal reads is the Container the catalog
    /// says the Entry lives in.
    pub async fn packed_library() -> Self {
        Self::mapping(None, true).await.started().await
    }

    /// A server over a device that maps only one top-level component
    /// (spec: EP-9).
    ///
    /// Everything outside it is in the catalog and reaches no folder here, which
    /// is what an unmapped Entry is.
    pub async fn mapping_only(prefix: &str) -> Self {
        Self::mapping(Some(EntryPath::nfc(prefix)), false)
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
        Self::mapping(None, false).await
    }

    async fn mapping(prefix: Option<EntryPath>, packed: bool) -> Self {
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
            .set_mapping(Mapping {
                prefix: None,
                local_root: remote.path().to_path_buf(),
                root_identity: None,
            })
            .await
            .expect("a mapping is recorded");
        let outcome = sync_folders(SyncRequest::new(
            store.as_ref(),
            &filled,
            &keys,
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
                prefix: Some(EntryPath::nfc("books")),
                ..FreezeRequest::new(
                    store.as_ref(),
                    &filled,
                    &keys,
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
            .set_mapping(Mapping {
                prefix: prefix.clone(),
                local_root: local.path().to_path_buf(),
                root_identity: None,
            })
            .await
            .expect("a mapping is recorded");

        let library = OpenLibrary {
            store: Arc::clone(&reads) as Arc<dyn ObjectStore>,
            index: Arc::new(index),
            keys,
            spool: spools.path().join("served"),
            library_id: LibraryId::from_bytes([0x11; LibraryId::BYTE_LEN]),
            epoch: MasterKeyEpoch::FIRST,
            provider: "s3",
        };

        let state = Arc::new(ServerState::new("served".to_owned(), library));
        let admission = Arc::new(Admission::new(AUTHORITY, SERVER_KEY));
        Self {
            router: router(Arc::clone(&state), admission),
            state,
            reads,
            storage,
            filled,
            store,
            local,
            remote,
            spools,
            batches: AtomicUsize::new(0),
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
            self.spools.path().join("filled"),
            // Named apart from the runs the fixture itself made, which the same
            // catalog holds the spool rows of (spec: OC-2).
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

    /// Asks one route, as the service it is.
    pub async fn get(&self, uri: &str) -> Response<Body> {
        self.send(
            asking("GET", uri)
                .body(Body::empty())
                .expect("a request with no body is well formed"),
        )
        .await
    }

    /// Asks one route twice at once.
    pub async fn get_twice(&self, uri: &str) -> (Response<Body>, Response<Body>) {
        tokio::join!(self.get(uri), self.get(uri))
    }

    /// Posts to one route, as the service it is.
    pub async fn post(&self, uri: &str) -> Response<Body> {
        self.send(
            asking("POST", uri)
                .body(Body::empty())
                .expect("a request with no body is well formed"),
        )
        .await
    }

    /// Drives the router with a request a case built for itself.
    ///
    /// For the cases about who is answered at all, which are the only ones that
    /// have anything to say about the headers: everything else asks through
    /// [`asking`], which sends what the explorer sends.
    pub async fn send(&self, request: Request<Body>) -> Response<Body> {
        self.router
            .clone()
            .oneshot(request)
            .await
            .expect("the router answers every request")
    }

    /// Drops files onto one folder, as a browser sends them.
    ///
    /// Each part carries its path relative to the folder as its filename, which
    /// is what a plain file drop and a folder drop both look like on the wire.
    pub async fn upload(&self, folder: &str, parts: &[(&str, &[u8])]) -> Response<Body> {
        self.dropped(folder, parts, false).await
    }

    /// The same, as a book being brought into a folder made for it.
    ///
    /// One parameter apart from an ordinary drop, and the whole of the
    /// difference on the wire: what it arms is a freeze of that folder rather
    /// than a sync (spec: PK-17).
    pub async fn upload_book(&self, folder: &str, parts: &[(&str, &[u8])]) -> Response<Body> {
        self.dropped(folder, parts, true).await
    }

    async fn dropped(&self, folder: &str, parts: &[(&str, &[u8])], freeze: bool) -> Response<Body> {
        let mut body: Vec<u8> = Vec::new();
        for (name, content) in parts {
            body.extend_from_slice(
                format!(
                    "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; \
                     filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
                )
                .as_bytes(),
            );
            body.extend_from_slice(content);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());

        let mut uri = match folder {
            "" => "/api/upload".to_owned(),
            named => format!("/api/upload?path={named}"),
        };
        if freeze {
            uri.push_str(match folder {
                "" => "?freeze=true",
                _ => "&freeze=true",
            });
        }
        self.send(
            asking("POST", &uri)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={BOUNDARY}"),
                )
                .body(Body::from(body))
                .expect("a multipart request is well formed"),
        )
        .await
    }

    /// Waits for the background sync to finish, whatever it came to.
    ///
    /// No sleep, and no polling: an upload arms the sync before it answers, so a
    /// case whose files have landed has already put the run on the state it waits
    /// on here.
    pub async fn sync_settled(&self) {
        self.state.syncs.settled().await;
    }

    /// Arms a fill without going through a route.
    ///
    /// Two of these back to back, with nothing awaited in between, is a fill
    /// superseded before it began — the one way to state "latest wins" as a
    /// case, since anything that awaits gives the worker a chance to run and
    /// leaves what it managed first up to the scheduler.
    pub fn arm_fill(&self, folder: &str) {
        let named = (!folder.is_empty()).then(|| EntryPath::nfc(folder));
        fill_folder(Arc::clone(&self.state), Folder::named(named));
    }

    /// Waits for the background fill to finish, whatever it came to.
    ///
    /// No sleep, and no polling: arming is synchronous, so a case that has asked
    /// for a file has already put the fill on the state it waits on here.
    pub async fn fill_settled(&self) {
        self.state.fills.settled().await;
    }

    /// Arms a freeze without going through a route.
    ///
    /// Two of these back to back, with nothing awaited in between, is a second
    /// book asked for while the first is still being packed — the one way to
    /// state "it waits its turn" as a case, since anything that awaits gives the
    /// worker a chance to finish and leaves the ordering up to the scheduler.
    pub fn arm_freeze(&self, folder: &str) {
        let named = (!folder.is_empty()).then(|| EntryPath::nfc(folder));
        freeze_folder(Arc::clone(&self.state), Folder::named(named));
    }

    /// Waits for the background freeze to finish, whatever it came to.
    ///
    /// No sleep, and no polling: a book drop arms the freeze before it answers,
    /// so a case whose pages have landed has already put the run on the state it
    /// waits on here.
    pub async fn freeze_settled(&self) {
        self.state.freezes.settled().await;
    }

    /// How many reads asked for part of an object, since the fixture was built.
    pub fn ranged_reads(&self) -> usize {
        self.reads.ranged_reads()
    }

    /// Takes Storage away, as an unreachable bucket or a grant that ran out.
    pub fn halt_storage(&self) {
        self.storage.halt();
    }

    /// Gives it back.
    pub fn resume_storage(&self) {
        self.storage.resume();
    }

    /// How many reads Storage refused while it was away.
    pub fn refused_reads(&self) -> usize {
        self.storage.refused()
    }

    /// Takes every read and answers none of it, until it is let go.
    ///
    /// What a case uses this for is a request it knows is inside the server: it
    /// waits for [`held_reads`](Self::held_reads) to move, does whatever it is
    /// about to the server, and then lets the read go and reads the answer.
    pub fn hold_storage(&self) {
        self.storage.hold();
    }

    /// Lets the held read go.
    pub fn release_storage(&self) {
        self.storage.release();
    }

    /// How many reads are being, or have been, held.
    pub fn held_reads(&self) -> usize {
        self.storage.held_reads()
    }

    /// Watches for the idle interval, as the binary does beside the socket.
    ///
    /// The clock is the case's own: every case over this runs with time paused,
    /// so a quarter of an hour of quiet is stated rather than spent. The yield
    /// is what puts the watcher on its first sleep before the case moves the
    /// clock — without it the first advance would be one nothing was waiting on.
    ///
    /// The handle is what a case asking whether the watcher is still there
    /// reads: a task that panicked is a finished task, and a watcher that had
    /// panicked would leave a Library that stays open and a case that could not
    /// tell that from one being kept open on purpose.
    pub async fn watch_idle(&self, after: Duration) -> JoinHandle<()> {
        let watcher = tokio::spawn(lock_when_idle(Arc::clone(&self.state), after));
        tokio::task::yield_now().await;
        watcher
    }

    /// Leaves Storage reachable and mute, as a filtered network does.
    ///
    /// The other half of [`halt_storage`](Self::halt_storage): nothing is
    /// refused, and nothing is answered either, so whatever asked waits until it
    /// decides not to. [`resume_storage`](Self::resume_storage) is how it stops.
    pub fn stall_storage(&self) {
        self.storage.stall();
    }

    /// How many reads Storage was asked for and never answered.
    pub fn stalled_reads(&self) -> usize {
        self.storage.stalled_reads()
    }

    /// Puts a file into the mapped folder that this device did not place there.
    pub fn plant_locally(&self, path: &str, content: &[u8]) {
        plant(self.local.path(), path, content);
    }

    /// Whether the mapped folder holds a file for one Entry Path.
    pub fn holds(&self, path: &str) -> bool {
        self.local_path(path).is_file()
    }

    /// Where in the mapped folder one Entry's file belongs (spec: EP-9).
    pub fn local_path(&self, path: &str) -> std::path::PathBuf {
        self.local.path().join(path)
    }
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
        .header(CAPABILITY_HEADER, SERVER_KEY)
}

/// What every multipart body a case sends is delimited by.
const BOUNDARY: &str = "coffret-case-boundary";

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
