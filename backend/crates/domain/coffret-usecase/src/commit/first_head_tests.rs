//! A Library's first head, as creating the Library writes it, and what reading
//! the Library makes of it.
//!
//! `head-0` adds nothing and removes nothing, so what is worth checking is the
//! slot it carries — in the form the Storage at hand consumes — and that every
//! reader treats it as the head it is: a catch-up from nothing stands there and
//! catalogs nothing, and a checkpoint of it is one to start from
//! (spec: CP-2, FM-13, RV-1, CK-9).

use coffret_format::{encode_index_snapshot, IndexSnapshotPayload};
use coffret_model::{ControlObjectKind, ControlObjectName, Generation, JournalRecord};

use super::control_fixtures::{control_keys, once, store_control, PAGE_SIZE};
use super::{catch_up, write_first_head, CommitPolicy};
use crate::in_memory_index::InMemoryIndex;
use crate::in_memory_minting_store::InMemoryMintingStore;
use crate::in_memory_store::InMemoryStore;
use crate::index::Index;
use crate::object_store::ObjectStore;

/// The names of the heads a store lists, in the order it lists them.
async fn heads(store: &dyn ObjectStore) -> Vec<String> {
    let mut heads = Vec::new();
    let mut token = None;
    loop {
        let page = store
            .list(token.as_ref())
            .await
            .expect("a store in memory lists");
        heads.extend(
            page.objects
                .into_iter()
                .map(|object| object.name)
                .filter(|name| {
                    matches!(
                        ControlObjectName::parse(name),
                        Ok(ControlObjectName::Head { .. })
                    )
                }),
        );
        token = page.next;
        if token.is_none() {
            return heads;
        }
    }
}

/// Writes the first head into an empty store, which has to write one.
async fn created(store: &dyn ObjectStore) -> JournalRecord {
    write_first_head(store, &control_keys(), &CommitPolicy::default())
        .await
        .expect("writing the first head into an empty store must succeed")
        .expect("an empty store holds no head to find instead")
}

/// What every first head is, whatever Storage it went to: the record at
/// generation 0, succeeding nothing, adding and removing nothing, and
/// selecting the Keyring's first generation (spec: FM-13, FM-15).
fn assert_empty_first_head(record: &JournalRecord) {
    assert_eq!(record.generation(), Generation::FIRST);
    assert_eq!(record.prev(), None);
    assert!(record.additions().is_empty());
    assert!(record.removals().is_empty());
    assert_eq!(record.keyring().generation(), Generation::FIRST);
}

// CP-2, CP-15, FM-13: on a Storage that keys objects by name the first head
// persists no slot — `head-1` is its successor's slot by name.
#[tokio::test]
async fn the_first_head_on_a_name_keyed_store_carries_its_slot_by_name() {
    let store = InMemoryStore::new(PAGE_SIZE);

    let record = created(&store).await;

    assert_empty_first_head(&record);
    assert_eq!(record.next_commit_slot(), None);
    assert_eq!(record.snapshot_slot(), None);
    assert_eq!(heads(&store).await, ["head-0.cfrt"]);
}

// CP-2, CP-3, FM-13: on a Storage that mints identifiers the first head carries
// the identifier minted for its successor, which is the one slot every writer
// of the Library's first commit consumes.
#[tokio::test]
async fn the_first_head_on_a_minting_store_carries_a_minted_slot() {
    let store = InMemoryMintingStore::new(PAGE_SIZE);

    let record = created(&store).await;

    assert_empty_first_head(&record);
    assert!(record.next_commit_slot().is_some());
    assert!(record.snapshot_slot().is_some());
    assert_eq!(heads(&store).await, ["head-0.cfrt"]);
}

// A Library that has a head is given no second first head: on a Storage that
// mints identifiers that would be two objects under `head-0` (spec: FM-12).
#[tokio::test]
async fn a_library_with_a_head_is_given_no_second_one() {
    let store = InMemoryMintingStore::new(PAGE_SIZE);
    created(&store).await;

    let again = write_first_head(&store, &control_keys(), &CommitPolicy::default())
        .await
        .expect("asking a Library that has a head must succeed");

    assert_eq!(again, None, "nothing is written where a head is listed");
    assert_eq!(heads(&store).await, ["head-0.cfrt"]);
}

// RV-1, CK-9: a device recovering a Library just created stands at its first
// head and catalogs nothing — and a checkpoint of that head is one to start
// from, adopted like any other.
#[tokio::test]
async fn a_library_just_created_recovers_to_nothing() {
    let store = InMemoryStore::new(PAGE_SIZE);
    let keys = control_keys();
    let first = created(&store).await;

    let index = InMemoryIndex::new();
    catch_up(&store, &index, &keys, &once())
        .await
        .expect("a Library just created is one to catch up with");
    let standing = index
        .snapshot()
        .await
        .expect("a caught-up catalog stands somewhere");
    assert_eq!(standing.checkpoint(), &first.checkpoint());
    assert!(standing.containers().is_empty());
    assert!(standing.entries().is_empty());
    assert_eq!(
        standing.adopted_from(),
        None,
        "there was no checkpoint to adopt"
    );

    // The same Library, checkpointed at its first head: a device starting from
    // nothing adopts the Snapshot and stands exactly where the replay stood.
    store_control(
        &store,
        &keys,
        &ControlObjectName::index_snapshot(Generation::FIRST),
        ControlObjectKind::IndexSnapshot,
        &encode_index_snapshot(&IndexSnapshotPayload::ordinary(standing.clone()))
            .expect("the Snapshot of a Library of nothing encodes"),
    )
    .await;
    let adopting = InMemoryIndex::new();
    catch_up(&store, &adopting, &keys, &once())
        .await
        .expect("a Library checkpointed at its first head is one to catch up with");
    let adopted = adopting
        .snapshot()
        .await
        .expect("a caught-up catalog stands somewhere");
    assert_eq!(
        adopted.adopted_from(),
        Some(&ControlObjectName::index_snapshot(Generation::FIRST)),
    );
    assert_eq!(adopted.checkpoint(), standing.checkpoint());
    assert!(adopted.containers().is_empty());
    assert!(adopted.entries().is_empty());
}
