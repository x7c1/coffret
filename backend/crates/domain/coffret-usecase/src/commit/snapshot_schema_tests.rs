//! What a catch-up starts from when the newest checkpoint predates entry
//! numbers (spec: FM-16).
//!
//! A Snapshot of schema 2 lists its Entries in Entry Path order and carries no
//! entry number for any of them, and the position of an Entry in its
//! Container's table is not something its path order says. So such a Snapshot
//! is passed over as a starting point, and a device with nothing better
//! rebuilds its catalog by replaying every Journal record from `head-0`, which
//! numbers each Entry by its place in the addition that carried it. A Snapshot
//! of schema 3 carries the numbers and is adopted as before. These cases hold
//! the two paths side by side over one Library.

use ciborium::Value;
use coffret_format::{
    encode_index_snapshot, encode_journal_record, ControlPayload, IndexSnapshotPayload,
};
use coffret_model::{
    CiphertextLenClaim, ContainerAddition, ContainerId, ContainerKind, ContainerSummary,
    ContentHash, ControlObjectKind, ControlObjectName, EntryExtent, EntryMetadata, EntryPath,
    Generation, JournalRecord, MasterKeyEpoch, Mtime, SnapshotContent,
};

use super::catch_up;
use super::control_fixtures::{
    commitment, control_keys, once, record_at, store_control, PAGE_SIZE,
};
use super::ControlKeys;
use crate::generations::generation;
use crate::in_memory_index::InMemoryIndex;
use crate::in_memory_store::InMemoryStore;
use crate::index::Index;

/// The registry's field numbers this rewrite touches
/// (`docs/spec/format/field-numbers.json`): `index_snapshot.schema`,
/// `index_snapshot.entries`, and `snapshot_entry.entry`.
const SNAPSHOT_SCHEMA_FIELD: u64 = 1;
const SNAPSHOT_ENTRIES_FIELD: u64 = 9;
const SNAPSHOT_ENTRY_NUMBER_FIELD: u64 = 10;

/// The Pack the Library's second head adds: three Entries whose table order is
/// not their path order, so a number guessed from where an Entry sorts would be
/// the wrong one.
fn pack() -> ContainerAddition {
    let id = ContainerId::from_bytes([0x40; ContainerId::BYTE_LEN]);
    let entries = ["albums/c.jpg", "albums/a.jpg", "albums/b.jpg"]
        .iter()
        .enumerate()
        .map(|(position, text)| {
            let path = EntryPath::parse(*text).expect("a fixture holds a literal Entry Path");
            EntryMetadata {
                path: path.clone(),
                captured_path: path,
                extent: EntryExtent::new(position as u64 * 10, 10)
                    .expect("a fixture lays its Entries inside the address space"),
                mtime: Mtime::from_unix_seconds(1_700_000_000),
                btime: None,
                hash: ContentHash::from_bytes([position as u8; ContentHash::BYTE_LEN]),
                derived_from: None,
                mime: None,
            }
        })
        .collect();
    ContainerAddition::new(
        ContainerSummary {
            id,
            kind: ContainerKind::Pack,
            ciphertext_hash: ContentHash::from_bytes([0x40; ContentHash::BYTE_LEN]),
            ciphertext_len: CiphertextLenClaim::new(4096)
                .expect("a fixture holds a literal length"),
            object_ref: None,
        },
        entries,
    )
    .expect("a fixture holds a table that tiles")
}

/// The record at generation 1, adding [`pack`].
fn adding_the_pack() -> JournalRecord {
    JournalRecord::new(
        generation(1),
        Some(Generation::FIRST),
        MasterKeyEpoch::FIRST,
        commitment(),
        None,
        None,
        vec![pack()],
        Vec::new(),
    )
    .expect("a fixture holds a record succeeding the first head")
}

/// The entry numbers the catalog holds, by Entry Path.
async fn numbers(index: &InMemoryIndex) -> Vec<(String, u32)> {
    index
        .entries_under(None)
        .await
        .expect("listing the catalog must succeed")
        .into_iter()
        .map(|location| {
            (
                location.entry.path.as_str().to_owned(),
                location.entry_number.get(),
            )
        })
        .collect()
}

/// What replaying [`adding_the_pack`] numbers its Entries as.
fn by_table_position() -> Vec<(String, u32)> {
    vec![
        ("albums/a.jpg".to_owned(), 1),
        ("albums/b.jpg".to_owned(), 2),
        ("albums/c.jpg".to_owned(), 0),
    ]
}

/// A Library of three heads — an empty one, the one adding [`pack`], and an
/// empty one after it — checkpointed at the second by `snapshot`.
async fn library(store: &InMemoryStore, keys: &ControlKeys, snapshot: ControlPayload) {
    let records = [
        record_at(Generation::FIRST),
        adding_the_pack(),
        record_at(generation(2)),
    ];
    for record in &records {
        store_control(
            store,
            keys,
            &ControlObjectName::head(record.generation()),
            ControlObjectKind::Journal,
            &encode_journal_record(record).expect("a fixture record encodes"),
        )
        .await;
    }
    store_control(
        store,
        keys,
        &ControlObjectName::index_snapshot(generation(1)),
        ControlObjectKind::IndexSnapshot,
        &snapshot,
    )
    .await;
}

/// The checkpoint of the second head as this build writes it, at schema 3.
fn snapshot_of_the_second_head() -> ControlPayload {
    let record = adding_the_pack();
    let (container, entries) = pack().into_locations();
    let content = SnapshotContent::canonical(record.checkpoint(), None, vec![container], entries)
        .expect("the Library after the second head is one an Index could stand at");
    encode_index_snapshot(&IndexSnapshotPayload::ordinary(content))
        .expect("a fixture Snapshot encodes")
}

/// The same checkpoint as a build before entry numbers wrote it: schema 2, and
/// no `entry` in any Snapshot entry.
fn schema_2(payload: ControlPayload) -> ControlPayload {
    let mut body: Value =
        ciborium::from_reader(payload.body.as_slice()).expect("a payload body is CBOR");
    let Value::Map(fields) = &mut body else {
        panic!("a payload body is a map");
    };
    for (key, value) in fields.iter_mut() {
        if *key == Value::from(SNAPSHOT_SCHEMA_FIELD) {
            *value = Value::from(2u64);
        }
        if *key == Value::from(SNAPSHOT_ENTRIES_FIELD) {
            let Value::Array(entries) = value else {
                panic!("entries is an array");
            };
            for entry in entries {
                let Value::Map(entry) = entry else {
                    panic!("an entry is a map");
                };
                entry.retain(|(key, _)| *key != Value::from(SNAPSHOT_ENTRY_NUMBER_FIELD));
            }
        }
    }
    let mut bytes = Vec::new();
    ciborium::into_writer(&body, &mut bytes).expect("a map of values serializes");
    ControlPayload::new(payload.master_key_epoch, bytes)
}

// FM-16: a Snapshot of schema 3 carries each Entry's number, so a device
// catching up from nothing starts from it, and the numbers it holds afterwards
// are the Snapshot's.
#[tokio::test]
async fn a_schema_3_snapshot_is_adopted_with_its_entry_numbers() {
    let store = InMemoryStore::new(PAGE_SIZE);
    let keys = control_keys();
    library(&store, &keys, snapshot_of_the_second_head()).await;

    let index = InMemoryIndex::new();
    catch_up(&store, &index, &keys, &once())
        .await
        .expect("a catch-up over a sound Library succeeds");

    let standing = index
        .snapshot()
        .await
        .expect("the catalog stands somewhere");
    assert_eq!(
        standing.adopted_from(),
        Some(&ControlObjectName::index_snapshot(generation(1))),
        "the checkpoint is what the catalog was started from",
    );
    assert_eq!(standing.checkpoint().head_generation(), generation(2));
    assert_eq!(numbers(&index).await, by_table_position());
}

// FM-16: a Snapshot of schema 2 carries no entry numbers, and nothing may guess
// them from its path order. So it is passed over, and the catalog is rebuilt by
// replaying every record from `head-0` — which numbers each Entry by its place
// in the table its addition carried.
#[tokio::test]
async fn a_schema_2_snapshot_is_passed_over_for_a_replay_from_the_first_head() {
    let store = InMemoryStore::new(PAGE_SIZE);
    let keys = control_keys();
    library(&store, &keys, schema_2(snapshot_of_the_second_head())).await;

    let index = InMemoryIndex::new();
    catch_up(&store, &index, &keys, &once())
        .await
        .expect("a checkpoint too old to start from does not stop a catch-up");

    let standing = index
        .snapshot()
        .await
        .expect("the catalog stands somewhere");
    assert_eq!(
        standing.adopted_from(),
        None,
        "nothing was adopted: the catalog was replayed from the first head",
    );
    assert_eq!(standing.checkpoint().head_generation(), generation(2));
    assert_eq!(numbers(&index).await, by_table_position());
}
