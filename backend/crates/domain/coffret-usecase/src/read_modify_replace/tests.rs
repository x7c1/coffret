//! What a rebuild makes of a catalog table that the conformance suite's cases
//! cannot arrange through a flow: one that is not the table the Container
//! carries, one whose row the Library holds at a renamed path and with another
//! modification time, and a Container with no object.

use std::path::Path;

use coffret_format::{
    generate_container_id, generate_container_key, wrap_container_key, ContainerWriter, EncodePlan,
    EntryPlan,
};
use coffret_model::{
    ContainerKind, ContainerSummary, ContentHash, EntryMetadata, KeyEnvelope, MasterKey,
    MasterKeyEpoch, Mtime,
};

use super::{rebuild, Rebuilding, Replacing, Unverified};
use crate::byte_stream::ByteStream;
use crate::ciphertext_len_claims::ciphertext_len;
use crate::commit::ControlListing;
use crate::device_state::{BatchId, DeviceTime};
use crate::entry_paths::entry_path;
use crate::in_memory_fs::InMemoryFs;
use crate::in_memory_index::InMemoryIndex;
use crate::in_memory_store::InMemoryStore;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;
use crate::spool::Spool;
use crate::spooled_container::SpooledContainer;

const SPOOL_DIR: &str = "/spool";

fn keys() -> LibraryKeys {
    LibraryKeys::derive(
        &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
        MasterKeyEpoch::FIRST,
    )
}

/// A Pack of these files, written and put on Storage the way a freeze would,
/// with what the catalog would record about it.
async fn stored_pack(
    store: &dyn ObjectStore,
    keys: &LibraryKeys,
    files: &[(&str, &[u8])],
) -> (ContainerSummary, KeyEnvelope, Vec<EntryMetadata>) {
    let container_id = generate_container_id().expect("the OS CSPRNG is available");
    let key = generate_container_key().expect("the OS CSPRNG is available");
    let plans: Vec<EntryPlan> = files
        .iter()
        .map(|(path, content)| {
            EntryPlan::new(
                entry_path(*path),
                Mtime::from_unix_seconds(1_700_000_000),
                content.len() as u64,
                ContentHash::from_bytes(*blake3::hash(content).as_bytes()),
            )
        })
        .collect();
    let mut object = Vec::new();
    let mut writer = ContainerWriter::begin(
        &EncodePlan::new(container_id, ContainerKind::Pack, &key, &plans),
        &mut object,
    )
    .expect("the plan is written");
    for (_, content) in files {
        writer
            .write(content, &mut object)
            .expect("the content is fed");
    }
    let table = writer.finish(&mut object).expect("the Pack closes");
    let uploaded = store
        .put(
            &container_id.object_name(),
            ByteStream::from(object.clone()),
        )
        .await
        .expect("putting a Pack must succeed");
    let summary = ContainerSummary {
        id: container_id,
        kind: ContainerKind::Pack,
        ciphertext_hash: ContentHash::from_bytes(*blake3::hash(&object).as_bytes()),
        ciphertext_len: ciphertext_len(object.len() as u64),
        object_ref: Some(uploaded.object_ref),
    };
    let envelope = wrap_container_key(keys.container_wrap(), &container_id, &key)
        .expect("wrapping a fresh key must succeed");
    (summary, envelope, table)
}

/// Rebuilds `summary` on a fresh device keeping the rows `keep` names.
async fn rebuild_keeping(
    store: &dyn ObjectStore,
    index: &dyn Index,
    fs: &InMemoryFs,
    keys: &LibraryKeys,
    (summary, envelope, table): (&ContainerSummary, &KeyEnvelope, &[EntryMetadata]),
    keep: &[bool],
) -> Result<SpooledContainer, Unverified> {
    let listing = ControlListing::default();
    let retry = RetryPolicy::default();
    let batch = BatchId::new("a-deletion");
    fs.prepare_dir(Path::new(SPOOL_DIR))
        .await
        .expect("preparing the spool directory must succeed");
    rebuild(
        &Rebuilding {
            store,
            index,
            keys,
            spool: fs,
            spool_dir: Path::new(SPOOL_DIR),
            retry: &retry,
            listing: &listing,
            batch: &batch,
            now: DeviceTime::from_unix_seconds(1_700_000_100),
        },
        &Replacing {
            old: summary,
            envelope,
            table,
            keep,
        },
    )
    .await
    .expect("a refusal is the Container's verdict, not the run's")
}

/// PK-10, CP-11: a Container whose own meta section does not carry the table
/// the catalog records for it is refused, and the replacement the rebuild had
/// begun is gone with the row that named it (spec: OC-2, OC-8).
///
/// Each row is held against the meta entry at its entry number, on its
/// captured path and its hash as well as its extent: a catalog naming another
/// Entry at that number, or other content for it, is refused alike. The object
/// is exactly what its record hashes to, so this is not damage in transit: it
/// is a catalog that cannot be trusted to say which Entries a replacement
/// should carry, and a replacement written from it would carry whatever that
/// catalog says.
#[tokio::test]
async fn a_table_the_container_does_not_carry_is_refused_and_leaves_nothing() {
    type Disagreement = fn(&mut EntryMetadata);
    let disagreements: [(&str, Disagreement); 2] = [
        ("captured path", |row| {
            row.captured_path = entry_path("albums/c.jpg");
        }),
        ("hash", |row| {
            row.hash = ContentHash::from_bytes(*blake3::hash(b"other").as_bytes());
        }),
    ];
    for (what, disagree) in disagreements {
        let store = InMemoryStore::new(16);
        let index = InMemoryIndex::new();
        let fs = InMemoryFs::new();
        let keys = keys();
        let (summary, envelope, mut table) = stored_pack(
            &store,
            &keys,
            &[("albums/a.jpg", b"kept"), ("albums/b.jpg", b"deleted")],
        )
        .await;
        disagree(&mut table[0]);

        let rebuilt = rebuild_keeping(
            &store,
            &index,
            &fs,
            &keys,
            (&summary, &envelope, &table),
            &[true, false],
        )
        .await;

        assert!(
            matches!(rebuilt, Err(Unverified::Disagrees)),
            "a row of another {what} is not the Container's, got {rebuilt:?}",
        );
        assert!(
            fs.files_beneath(Path::new(SPOOL_DIR)).is_empty(),
            "the replacement it had begun is gone",
        );
        assert!(
            index
                .pending_rows()
                .await
                .expect("asking for pending rows must succeed")
                .is_empty(),
            "and so is the row that named it",
        );
    }
}

/// PK-10, FM-15: a row the catalog holds at a renamed path is still the
/// Entry its Container records at that entry number, so the Pack is carried
/// forward, and the replacement records the kept Entry under its current
/// path: a rewrite captures the name current at the rewrite.
///
/// What a rename leaves alone and a Container does not record is not held
/// against the table either: a row whose modification time the Library holds
/// differently is carried with the time the catalog holds (spec: FM-9).
#[tokio::test]
async fn a_renamed_row_is_carried_forward_under_its_current_path() {
    let store = InMemoryStore::new(16);
    let index = InMemoryIndex::new();
    let fs = InMemoryFs::new();
    let keys = keys();
    let (summary, envelope, mut table) = stored_pack(
        &store,
        &keys,
        &[("albums/a.jpg", b"kept"), ("albums/b.jpg", b"deleted")],
    )
    .await;
    table[0].path = entry_path("books/a.jpg");
    table[0].mtime = Mtime::from_unix_seconds(1_800_000_000);

    let rebuilt = rebuild_keeping(
        &store,
        &index,
        &fs,
        &keys,
        (&summary, &envelope, &table),
        &[true, false],
    )
    .await
    .unwrap_or_else(|unverified| panic!("a renamed row is the Container's: {unverified}"));

    let [carried] = rebuilt.entries.as_slice() else {
        panic!("one Entry is kept, got {:?}", rebuilt.entries);
    };
    assert_eq!(carried.path, entry_path("books/a.jpg"));
    assert_eq!(
        carried.captured_path, carried.path,
        "the replacement captures the name current at the rewrite",
    );
    assert_eq!(carried.hash, table[0].hash);
    assert_eq!(carried.mtime, table[0].mtime);
}

/// FM-3: a Container Storage holds no object for has nothing to rebuild from,
/// and the refusal comes before anything is announced.
#[tokio::test]
async fn a_container_with_no_object_is_unreachable() {
    let store = InMemoryStore::new(16);
    let index = InMemoryIndex::new();
    let fs = InMemoryFs::new();
    let keys = keys();
    let (mut summary, envelope, table) = stored_pack(
        &store,
        &keys,
        &[("albums/a.jpg", b"kept"), ("albums/b.jpg", b"gone")],
    )
    .await;
    // Neither a cached handle nor a listing that names it.
    summary.object_ref = None;

    let listing = ControlListing::default();
    let retry = RetryPolicy::default();
    let batch = BatchId::new("a-deletion");
    let rebuilt = rebuild(
        &Rebuilding {
            store: &store,
            index: &index,
            keys: &keys,
            spool: &fs,
            spool_dir: Path::new(SPOOL_DIR),
            retry: &retry,
            listing: &listing,
            batch: &batch,
            now: DeviceTime::from_unix_seconds(1_700_000_100),
        },
        &Replacing {
            old: &summary,
            envelope: &envelope,
            table: &table,
            keep: &[true, false],
        },
    )
    .await
    .expect("a refusal is the Container's verdict, not the run's");

    assert!(matches!(rebuilt, Err(Unverified::Unreachable)));
    assert!(index
        .pending_rows()
        .await
        .expect("asking for pending rows must succeed")
        .is_empty());
}
