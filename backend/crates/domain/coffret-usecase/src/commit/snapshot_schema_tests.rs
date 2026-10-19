//! What a catch-up starts from, and what it holds afterwards, when the newest
//! checkpoint is a Snapshot (spec: FM-16).
//!
//! A Snapshot of this build's schema carries each Entry's entry number and
//! deposit time, so a device catching up from nothing adopts it and holds what
//! a replay of every record would have left — paths, numbers and deposit times
//! alike. A Snapshot of an earlier schema carries no deposit time, and neither
//! do the records it checkpoints, so it is refused rather than adopted with a
//! time made up. These cases hold both over one Library.

use ciborium::Value;
use coffret_format::{
    encode_index_snapshot, encode_journal_record, ControlPayload, IndexSnapshotPayload,
};
use coffret_model::{
    CiphertextLenClaim, ContainerAddition, ContainerId, ContainerKind, ContainerSummary,
    ContentHash, ControlObjectKind, ControlObjectName, DepositedAt, EntryExtent, EntryMetadata,
    EntryNumber, EntryPath, EntryRename, Generation, JournalRecord, MasterKeyEpoch, Mtime,
    SnapshotContent,
};

use super::catch_up;
use super::control_fixtures::{
    commitment, control_keys, once, record_at, store_control, PAGE_SIZE,
};
use super::{CommitError, ControlKeys, ControlObjectFault};
use crate::generations::generation;
use crate::in_memory_index::InMemoryIndex;
use crate::in_memory_store::InMemoryStore;
use crate::index::Index;

/// The registry's field numbers this rewrite touches
/// (`docs/spec/format/field-numbers.json`): `index_snapshot.schema`,
/// `index_snapshot.entries`, and `snapshot_entry.deposited_at`.
const SNAPSHOT_SCHEMA_FIELD: u64 = 1;
const SNAPSHOT_ENTRIES_FIELD: u64 = 9;
const SNAPSHOT_ENTRY_DEPOSITED_AT_FIELD: u64 = 11;

/// When the Entry at `position` of [`pack`] was deposited: a second of its own
/// for each, and in the opposite order to the table, so neither position nor
/// path order could stand in for it.
fn deposited_at(position: usize) -> DepositedAt {
    DepositedAt::from_unix_seconds(1_760_000_000 - position as i64 * 60)
}

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
        (0..3).map(deposited_at).collect(),
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

/// The deposit times the catalog holds, by Entry Path.
async fn deposit_times(index: &InMemoryIndex) -> Vec<(String, DepositedAt)> {
    index
        .entries_under(None)
        .await
        .expect("listing the catalog must succeed")
        .into_iter()
        .map(|location| {
            (
                location.entry.path.as_str().to_owned(),
                location.deposited_at,
            )
        })
        .collect()
}

/// What replaying [`adding_the_pack`] stamps its Entries with.
fn as_deposited() -> Vec<(String, DepositedAt)> {
    vec![
        ("albums/a.jpg".to_owned(), deposited_at(1)),
        ("albums/b.jpg".to_owned(), deposited_at(2)),
        ("albums/c.jpg".to_owned(), deposited_at(0)),
    ]
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

/// The checkpoint of the second head as this build writes it.
fn snapshot_of_the_second_head() -> ControlPayload {
    let record = adding_the_pack();
    let (container, entries) = pack().into_locations();
    let content = SnapshotContent::canonical(record.checkpoint(), None, vec![container], entries)
        .expect("the Library after the second head is one an Index could stand at");
    encode_index_snapshot(&IndexSnapshotPayload::ordinary(content))
        .expect("a fixture Snapshot encodes")
}

/// The same checkpoint as a build before deposit times wrote it: schema 3, and
/// no `deposited_at` in any Snapshot entry.
fn before_deposit_times(payload: ControlPayload) -> ControlPayload {
    let mut body: Value =
        ciborium::from_reader(payload.body.as_slice()).expect("a payload body is CBOR");
    let Value::Map(fields) = &mut body else {
        panic!("a payload body is a map");
    };
    for (key, value) in fields.iter_mut() {
        if *key == Value::from(SNAPSHOT_SCHEMA_FIELD) {
            *value = Value::from(3u64);
        }
        if *key == Value::from(SNAPSHOT_ENTRIES_FIELD) {
            let Value::Array(entries) = value else {
                panic!("entries is an array");
            };
            for entry in entries {
                let Value::Map(entry) = entry else {
                    panic!("an entry is a map");
                };
                entry.retain(|(key, _)| *key != Value::from(SNAPSHOT_ENTRY_DEPOSITED_AT_FIELD));
            }
        }
    }
    let mut bytes = Vec::new();
    ciborium::into_writer(&body, &mut bytes).expect("a map of values serializes");
    ControlPayload::new(payload.master_key_epoch, bytes)
}

// FM-16, RV-1: a Snapshot carries each Entry's number and deposit time, so a
// device catching up from nothing starts from it, and the numbers and times it
// holds afterwards are the Snapshot's — the ones the record that deposited the
// Entries stamped.
#[tokio::test]
async fn a_snapshot_is_adopted_with_its_entry_numbers_and_deposit_times() {
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
    assert_eq!(deposit_times(&index).await, as_deposited());
}

// FM-16: a Snapshot from before deposit times carries none, and nothing may
// make one up. So it is refused as a schema this build does not read, and the
// catch-up stops there with nothing adopted, rather than starting a catalog
// whose Entries have no deposit time.
#[tokio::test]
async fn a_snapshot_from_before_deposit_times_is_refused() {
    let store = InMemoryStore::new(PAGE_SIZE);
    let keys = control_keys();
    library(
        &store,
        &keys,
        before_deposit_times(snapshot_of_the_second_head()),
    )
    .await;

    let index = InMemoryIndex::new();
    let result = catch_up(&store, &index, &keys, &once()).await;
    assert!(
        matches!(
            &result,
            Err(CommitError::CorruptControlObject {
                fault: ControlObjectFault::Unopenable(
                    coffret_format::Error::UnsupportedIndexSnapshotSchema { schema: 3 }
                ),
                ..
            })
        ),
        "expected the old Snapshot to be refused, got {:?}",
        result.as_ref().err(),
    );
    assert!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .is_none(),
        "nothing was adopted",
    );
}

/// A record at `head` renaming `renames` of [`pack`]'s Entries, each given as
/// its entry number and the path it takes.
fn renaming_at(head: u64, renames: &[(u32, &str)]) -> JournalRecord {
    let id = pack().container().id;
    JournalRecord::new(
        generation(head),
        Some(generation(head - 1)),
        MasterKeyEpoch::FIRST,
        commitment(),
        None,
        None,
        Vec::new(),
        Vec::new(),
    )
    .and_then(|record| {
        record.with_renames(
            renames
                .iter()
                .map(|(number, path)| {
                    EntryRename::new(
                        id,
                        EntryNumber::new(u64::from(*number))?,
                        EntryPath::parse(*path).expect("a fixture holds a literal Entry Path"),
                    )
                })
                .collect::<Result<_, _>>()?,
        )
    })
    .expect("a fixture holds a record renaming the pack's Entries")
}

/// Every Entry's current and captured path, in Entry Path order.
async fn names(index: &InMemoryIndex) -> Vec<(String, String)> {
    index
        .entries_under(None)
        .await
        .expect("listing the catalog must succeed")
        .into_iter()
        .map(|location| {
            (
                location.entry.path.as_str().to_owned(),
                location.entry.captured_path.as_str().to_owned(),
            )
        })
        .collect()
}

// RV-1, RV-5, FM-15, FM-16: renames replay. A device replaying the whole
// Journal and one adopting a Snapshot taken between two renaming records, then
// replaying the record after it, stand at one catalog — current paths,
// captured paths, entry numbers and deposit times alike — because the Snapshot
// carries the numbers the later renames name their Entries by, and a rename
// carries an Entry's deposit time with it.
#[tokio::test]
async fn a_snapshot_and_the_renames_after_it_reach_what_a_full_replay_does() {
    let records = [
        record_at(Generation::FIRST),
        adding_the_pack(),
        // A swap and a third Entry leaving its path in one record…
        renaming_at(
            2,
            &[
                (0, "albums/b.jpg"),
                (1, "albums/z.jpg"),
                (2, "albums/c.jpg"),
            ],
        ),
        // …and, after the checkpoint, one renamed again and one renamed back.
        renaming_at(3, &[(0, "2024/c.jpg"), (1, "albums/a.jpg")]),
    ];
    let keys = control_keys();

    let replayed_store = InMemoryStore::new(PAGE_SIZE);
    let adopted_store = InMemoryStore::new(PAGE_SIZE);
    for store in [&replayed_store, &adopted_store] {
        for record in &records {
            store_control(
                store,
                &keys,
                &ControlObjectName::head(record.generation()),
                ControlObjectKind::Journal,
                &encode_journal_record(record).expect("a fixture record encodes"),
            )
            .await;
        }
    }
    // The checkpoint of the third head, as a device standing there writes it.
    let standing = InMemoryIndex::new();
    for record in &records[..3] {
        standing
            .apply(record.clone())
            .await
            .expect("the records up to the checkpoint apply");
    }
    store_control(
        &adopted_store,
        &keys,
        &ControlObjectName::index_snapshot(generation(2)),
        ControlObjectKind::IndexSnapshot,
        &encode_index_snapshot(&IndexSnapshotPayload::ordinary(
            standing
                .snapshot()
                .await
                .expect("the catalog stands at the third head"),
        ))
        .expect("a fixture Snapshot encodes"),
    )
    .await;

    let replayed = InMemoryIndex::new();
    catch_up(&replayed_store, &replayed, &keys, &once())
        .await
        .expect("a catch-up replaying every record succeeds");
    let adopted = InMemoryIndex::new();
    catch_up(&adopted_store, &adopted, &keys, &once())
        .await
        .expect("a catch-up from the checkpoint succeeds");

    let replayed_state = replayed
        .snapshot()
        .await
        .expect("the replay stands somewhere");
    let adopted_state = adopted
        .snapshot()
        .await
        .expect("the adoption stands somewhere");
    assert_eq!(replayed_state.adopted_from(), None);
    assert_eq!(
        adopted_state.adopted_from(),
        Some(&ControlObjectName::index_snapshot(generation(2))),
        "the second device started from the checkpoint",
    );
    assert_eq!(replayed_state.checkpoint(), adopted_state.checkpoint());
    assert_eq!(replayed_state.containers(), adopted_state.containers());
    assert_eq!(replayed_state.entries(), adopted_state.entries());
    assert_eq!(
        names(&replayed).await,
        [
            ("2024/c.jpg".to_owned(), "albums/c.jpg".to_owned()),
            ("albums/a.jpg".to_owned(), "albums/a.jpg".to_owned()),
            ("albums/c.jpg".to_owned(), "albums/b.jpg".to_owned()),
        ]
    );
    // Each Entry keeps the time the record adding the Pack stamped it with,
    // under whichever path the renames left it at.
    assert_eq!(
        deposit_times(&adopted).await,
        [
            ("2024/c.jpg".to_owned(), deposited_at(0)),
            ("albums/a.jpg".to_owned(), deposited_at(1)),
            ("albums/c.jpg".to_owned(), deposited_at(2)),
        ]
    );
}
