//! Which slot a commit and a checkpoint consume, on both shapes of Storage.
//!
//! The suite's cases take one store at a time, and these need two kinds of
//! store side by side: what a head carries is only checkable against a store
//! that mints identifiers, and a slot a head carries in the wrong form only
//! arises when a Library written to one shape of Storage is read on the other.
//! So they are this crate's own tests rather than suite cases, over the
//! in-memory store of each shape.

use coffret_model::{ContainerKind, ControlObjectName, JournalRecord};

use crate::byte_stream::ByteStream;
use crate::commit::{
    commit_batch, write_first_head, CommitError, ControlObjectFault, PreparedBatch,
};
use crate::commit_conformance::fixtures::{
    container_id, control_keys, policy, prepared, request, request_under,
};
use crate::commit_conformance::library::Library;
use crate::generations::generation;
use crate::in_memory_index::InMemoryIndex;
use crate::in_memory_minting_store::InMemoryMintingStore;
use crate::in_memory_store::InMemoryStore;
use crate::object_store::ObjectStore;

/// Small enough that a walk of either store reaches a second page.
const PAGE_SIZE: usize = 3;

/// One commit adding Container `seed` at one path of its own, with every
/// Snapshot written that the commit may write.
async fn commit_checkpointed(
    store: &dyn ObjectStore,
    index: &InMemoryIndex,
    seed: u8,
) -> JournalRecord {
    Library::upload_container(store, container_id(seed)).await;
    let batch = PreparedBatch::adding(vec![prepared(
        seed,
        ContainerKind::OneFile,
        &[&format!("files/{seed}.txt")],
    )]);
    let keys = control_keys();
    commit_batch(request_under(
        store,
        index,
        &keys,
        batch,
        policy().with_checkpoint_threshold(0),
    ))
    .await
    .unwrap_or_else(|failure| panic!("an uncontested commit must succeed: {failure:?}"))
    .record
}

/// Creates a Library in an empty store, returning its first head
/// (spec: CP-2, FM-13).
async fn created(store: &dyn ObjectStore) -> JournalRecord {
    write_first_head(store, &control_keys(), &policy(), None)
        .await
        .expect("creating a Library in an empty store must succeed")
        .expect("an empty store holds no head to find instead")
}

/// Puts every live object of `from` into `to` under the name it is listed by,
/// as a Library carried from one Storage to another would be.
async fn copy(from: &dyn ObjectStore, to: &dyn ObjectStore) {
    let library = Library::read(from).await;
    for name in library.names() {
        let object = library.handle_named(&name);
        let bytes = from
            .get(&object, None)
            .await
            .expect("a listed object must be readable")
            .into_bytes()
            .await
            .expect("the stream is as long as it claims");
        to.put(&name, ByteStream::from(bytes))
            .await
            .expect("storing a copied object must succeed");
    }
}

// CP-2, CK-10, CP-15: on a Storage that mints identifiers, the successor of a
// head is the object the head's `next_commit_slot` named and its checkpoint the
// one its `snapshot_slot` named — the one identifier every writer starting from
// that head aims at, not one this writer minted for itself.
#[tokio::test]
async fn a_minting_store_consumes_the_identifiers_the_head_carries() {
    let store = InMemoryMintingStore::new(PAGE_SIZE);
    let index = InMemoryIndex::new();

    let first_head = created(&store).await;
    let first = commit_checkpointed(&store, &index, 1).await;
    let second = commit_checkpointed(&store, &index, 2).await;

    let library = Library::read(&store).await;
    let carried = |slot: Option<&str>| {
        slot.expect("a record on a minting store carries its slots")
            .to_owned()
    };
    assert_eq!(
        library
            .handle(&ControlObjectName::head(generation(1)))
            .as_str(),
        carried(first_head.next_commit_slot()),
        "the first commit is the object the head the Library was created with named",
    );
    assert_eq!(
        library
            .handle(&ControlObjectName::head(generation(2)))
            .as_str(),
        carried(first.next_commit_slot()),
        "head 2 is the object head 1's next_commit_slot named",
    );
    assert_eq!(
        library
            .handle(&ControlObjectName::index_snapshot(generation(1)))
            .as_str(),
        carried(first.snapshot_slot()),
        "the checkpoint of head 1 is the object head 1's snapshot_slot named",
    );
    assert_eq!(
        library
            .handle(&ControlObjectName::index_snapshot(generation(2)))
            .as_str(),
        carried(second.snapshot_slot()),
        "the checkpoint of head 2 is the object head 2's snapshot_slot named",
    );
}

// CP-2, CP-15: where names identify objects a head persists no slot at all, and
// the successor and the checkpoint are created under the names re-derived from
// its generation.
#[tokio::test]
async fn a_name_keyed_store_consumes_the_names_the_head_derives() {
    let store = InMemoryStore::new(PAGE_SIZE);
    let index = InMemoryIndex::new();

    let first_head = created(&store).await;
    let first = commit_checkpointed(&store, &index, 1).await;
    commit_checkpointed(&store, &index, 2).await;

    for head in [&first_head, &first] {
        assert_eq!(head.next_commit_slot(), None);
        assert_eq!(head.snapshot_slot(), None);
    }
    let library = Library::read(&store).await;
    assert_eq!(
        library
            .handle(&ControlObjectName::head(generation(2)))
            .as_str(),
        "head-2.cfrt",
    );
    assert_eq!(
        library
            .handle(&ControlObjectName::index_snapshot(generation(1)))
            .as_str(),
        "idx-1.cfrt",
    );
}

/// What a commit onto a Library carried from `from` to `to` is refused with.
async fn refused_after_moving(from: &dyn ObjectStore, to: &dyn ObjectStore) -> CommitError {
    let keys = control_keys();
    created(from).await;
    commit_checkpointed(from, &InMemoryIndex::new(), 1).await;
    copy(from, to).await;

    Library::upload_container(to, container_id(2)).await;
    let batch = PreparedBatch::adding(vec![prepared(2, ContainerKind::OneFile, &["b.txt"])]);
    let failure = commit_batch(request(to, &InMemoryIndex::new(), &keys, batch))
        .await
        .expect_err("a head whose slot this Storage cannot consume is refused");

    assert_eq!(
        Library::read(to)
            .await
            .count(&ControlObjectName::head(generation(2))),
        0,
        "nothing was created in place of the slot the head did not carry",
    );
    *failure.error
}

// CP-2, CP-3: a head that carries no identifier on a Storage that mints them is
// refused rather than given a slot minted here, which would be this writer's
// alone and let a second writer on the same head succeed it as well.
#[tokio::test]
async fn a_head_with_no_carried_slot_on_a_minting_store_is_refused() {
    let error = refused_after_moving(
        &InMemoryStore::new(PAGE_SIZE),
        &InMemoryMintingStore::new(PAGE_SIZE),
    )
    .await;

    match error {
        CommitError::CorruptControlObject {
            object,
            fault: ControlObjectFault::NoCommitSlot,
        } => assert_eq!(object, ControlObjectName::head(generation(1))),
        other => panic!("expected the head to be refused for carrying no slot, got {other:?}"),
    }
}

// CP-2: and the other way round — an identifier is nothing a name-keyed Storage
// can create under, so a head that carries one there is refused too.
#[tokio::test]
async fn a_head_carrying_an_identifier_on_a_name_keyed_store_is_refused() {
    let error = refused_after_moving(
        &InMemoryMintingStore::new(PAGE_SIZE),
        &InMemoryStore::new(PAGE_SIZE),
    )
    .await;

    match error {
        CommitError::CorruptControlObject {
            object,
            fault: ControlObjectFault::MintedCommitSlot,
        } => assert_eq!(object, ControlObjectName::head(generation(1))),
        other => panic!("expected the head to be refused for its minted slot, got {other:?}"),
    }
}
