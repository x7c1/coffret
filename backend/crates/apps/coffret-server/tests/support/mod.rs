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
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use axum::Router;
use coffret_device::{EntryPath, OpenLibrary};
use coffret_model::{LibraryId, MasterKey, MasterKeyEpoch};
use coffret_server::{router, ServerState};
use coffret_usecase::device_state::{BatchId, DeviceTime, Mapping};
use coffret_usecase::sync::{sync_folders, SyncRequest};
use coffret_usecase::{InMemoryIndex, InMemoryStore, Index, LibraryKeys, ObjectStore};
use tempfile::TempDir;
use tower::ServiceExt;

mod counting_store;
use counting_store::CountingStore;

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
    reads: Arc<CountingStore>,
    /// The folder this device maps, so a case can put a file into it that the
    /// device did not place there.
    local: TempDir,
    /// Kept so that nothing the fixture made is removed while a case runs.
    _remote: TempDir,
    _spools: TempDir,
}

impl Served {
    /// A server over a device that maps the whole Library.
    pub async fn library() -> Self {
        Self::mapping(None).await
    }

    /// A server over a device that maps only one top-level component
    /// (spec: EP-9).
    ///
    /// Everything outside it is in the catalog and reaches no folder here, which
    /// is what an unmapped Entry is.
    pub async fn mapping_only(prefix: &str) -> Self {
        Self::mapping(Some(EntryPath::nfc(prefix))).await
    }

    async fn mapping(prefix: Option<EntryPath>) -> Self {
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

        // The device the server serves: the same Library, a folder of its own,
        // and nothing on disk yet.
        let reads = Arc::new(CountingStore::around(Arc::clone(&store)));
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

        // A fetch narrowed to a subtree the Library holds nothing under: it
        // catches the catalog up to the head — which is the first thing every
        // fetch does (spec: CK-9) — and places nothing. That is the state a
        // second enrolled device is in before it fetches anything.
        library
            .fetch(Some(EntryPath::nfc("nothing-of-this-name")))
            .await
            .expect("a device that has just joined catches up");
        reads.forget();

        Self {
            router: router(Arc::new(ServerState::new("served".to_owned(), library))),
            reads,
            local,
            _remote: remote,
            _spools: spools,
        }
    }

    /// Asks one route, as the service it is.
    pub async fn get(&self, uri: &str) -> Response<Body> {
        self.router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("a request with no body is well formed"),
            )
            .await
            .expect("the router answers every request")
    }

    /// Asks one route twice at once.
    pub async fn get_twice(&self, uri: &str) -> (Response<Body>, Response<Body>) {
        tokio::join!(self.get(uri), self.get(uri))
    }

    /// How many reads asked for part of an object, since the fixture was built.
    pub fn ranged_reads(&self) -> usize {
        self.reads.ranged_reads()
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
