//! A Library with no first head, and a Library with two control objects under
//! one name, on both shapes of Storage.
//!
//! The suite's cases all start from a Library as it is created, with its first
//! head written, so neither shape arises there. Both arise from Libraries that
//! exist all the same: one created before creating a Library wrote its first
//! head, and one whose exclusion did not hold — two writers racing for such a
//! Library's first head on a Storage that mints identifiers, or anything else
//! that put two successors of one head on Storage. So they are this crate's own
//! tests rather than suite cases, over the in-memory store of each shape.

use std::ops::Range;

use async_trait::async_trait;
use coffret_model::{ContainerKind, ControlObjectName, Generation, ObjectRef};

use crate::byte_stream::ByteStream;
use crate::commit::{catch_up, commit_batch, write_first_head, CommitError, PreparedBatch};
use crate::commit_conformance::fixtures::{
    container_id, control_keys, policy, prepared, request, request_under,
};
use crate::commit_conformance::library::{replica_name, Library};
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::generations::generation;
use crate::in_memory_index::InMemoryIndex;
use crate::in_memory_minting_store::InMemoryMintingStore;
use crate::in_memory_store::InMemoryStore;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::uploaded_object::UploadedObject;

/// Small enough that a walk of either store reaches a second page.
const PAGE_SIZE: usize = 3;

/// A batch adding Container `seed` at one path of its own, uploaded first.
async fn uploaded(store: &dyn ObjectStore, seed: u8) -> PreparedBatch {
    Library::upload_container(store, container_id(seed)).await;
    PreparedBatch::adding(vec![prepared(
        seed,
        ContainerKind::OneFile,
        &[&format!("files/{seed}.txt")],
    )])
}

/// Commits to a Library no first head was written into, and checks what it
/// leaves: an empty `head-0`, and the batch on top of it in `head-1`.
async fn a_head_less_library_is_committed_to(store: &dyn ObjectStore) {
    let keys = control_keys();
    let batch = uploaded(store, 1).await;

    let outcome = commit_batch(request(store, &InMemoryIndex::new(), &keys, batch))
        .await
        .unwrap_or_else(|failure| panic!("a Library with no head yet commits: {failure:?}"));

    assert_eq!(outcome.record.generation(), generation(1));
    assert_eq!(
        outcome.record.prev(),
        Some(Generation::FIRST),
        "the batch commits on top of the empty head written before it (spec: CP-2)",
    );
    assert_eq!(
        outcome.attempts, 1,
        "writing the first head is no attempt at the batch"
    );

    let library = Library::read(store).await;
    let first = library.record(store, Generation::FIRST).await;
    assert!(first.additions().is_empty() && first.removals().is_empty());
    assert_eq!(library.record(store, generation(1)).await, outcome.record);
    for generation in [Generation::FIRST, generation(1)] {
        assert_eq!(library.count(&ControlObjectName::head(generation)), 1);
    }
}

// CP-2, FM-13: a Library created before its first head was written at creation
// keeps working. Its first commit writes the empty `head-0` and then commits
// through the slot that head carries, as every later commit does.
#[tokio::test]
async fn a_head_less_library_on_a_name_keyed_store_is_committed_to() {
    a_head_less_library_is_committed_to(&InMemoryStore::new(PAGE_SIZE)).await;
}

#[tokio::test]
async fn a_head_less_library_on_a_minting_store_is_committed_to() {
    a_head_less_library_is_committed_to(&InMemoryMintingStore::new(PAGE_SIZE)).await;
}

/// A store whose listing never shows `head-0`, as a listing that has not caught
/// up with a create looks; everything else goes to the store underneath.
struct HidingFirstHead<'a> {
    inner: &'a dyn ObjectStore,
}

#[async_trait]
impl ObjectStore for HidingFirstHead<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<UploadedObject> {
        self.inner.put(name, body).await
    }

    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        self.inner.reserve_create(name).await
    }

    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        self.inner.put_if_absent(slot, body).await
    }

    fn object_at(&self, slot: &CommitSlot) -> Result<ObjectRef> {
        self.inner.object_at(slot)
    }

    async fn get(&self, object: &ObjectRef, range: Option<Range<u64>>) -> Result<ByteStream> {
        self.inner.get(object, range).await
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        let first_head = ControlObjectName::head(Generation::FIRST).to_string();
        let mut listed = self.inner.list(page).await?;
        listed.objects.retain(|object| object.name != first_head);
        Ok(listed)
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}

// CP-2, FM-12: a writer that wrote a head-less Library's empty `head-0` and
// does not find it listed afterwards stops rather than write another — on a
// Storage that mints identifiers a second one would sit beside the first under
// one name, and every reader would refuse the Library.
#[tokio::test]
async fn a_first_head_the_listing_does_not_show_is_not_written_again() {
    let store = InMemoryMintingStore::new(PAGE_SIZE);
    let lagging = HidingFirstHead { inner: &store };
    let batch = uploaded(&store, 1).await;

    let failure = commit_batch(request(
        &lagging,
        &InMemoryIndex::new(),
        &control_keys(),
        batch,
    ))
    .await
    .expect_err("a first head the listing does not show is not committed on top of");

    assert!(
        matches!(*failure.error, CommitError::FirstHeadUnlisted),
        "expected the run to stop on the unlisted first head, got {:?}",
        failure.error,
    );
    let library = Library::read(&store).await;
    assert_eq!(
        library.count(&ControlObjectName::head(Generation::FIRST)),
        1,
        "the first head was written once",
    );
    assert!(
        !library.holds(&ControlObjectName::head(generation(1))),
        "nothing of the batch was committed",
    );
}

/// Puts a second object under `name`, a copy of the one already there, as a
/// Storage that mints identifiers holds one where an exclusion did not hold.
async fn duplicate(store: &dyn ObjectStore, name: &str) {
    let bytes = Library::read(store).await.bytes(store, name).await;
    store
        .put(name, ByteStream::from(bytes))
        .await
        .expect("storing a copy must succeed");
}

/// A created Library on a minting store with one commit in it, checkpointed.
async fn committed_once(store: &InMemoryMintingStore) {
    write_first_head(store, &control_keys(), &policy())
        .await
        .expect("creating a Library in an empty store must succeed");
    let batch = uploaded(store, 1).await;
    commit_batch(request_under(
        store,
        &InMemoryIndex::new(),
        &control_keys(),
        batch,
        policy().with_checkpoint_threshold(0),
    ))
    .await
    .unwrap_or_else(|failure| panic!("an uncontested commit must succeed: {failure:?}"));
}

/// What the next commit, and a catch-up from nothing, make of a Library whose
/// listing has two objects under `name`.
async fn refused_with_two_of(name: &ControlObjectName) {
    let store = InMemoryMintingStore::new(PAGE_SIZE);
    committed_once(&store).await;
    duplicate(&store, &name.to_string()).await;
    let keys = control_keys();

    let batch = uploaded(&store, 2).await;
    let failure = commit_batch(request(&store, &InMemoryIndex::new(), &keys, batch))
        .await
        .expect_err("a Library with two control objects under one name is not committed to");
    match *failure.error {
        CommitError::DuplicateControlObject { ref object } => assert_eq!(object, name),
        ref other => panic!("expected the Library to be refused for {name}, got {other:?}"),
    }
    assert!(
        !Library::read(&store)
            .await
            .holds(&ControlObjectName::head(generation(2))),
        "the commit stopped before writing anything of its own",
    );

    let index = InMemoryIndex::new();
    match catch_up(&store, &index, &keys, &policy().retry).await {
        Err(CommitError::DuplicateControlObject { ref object }) => assert_eq!(object, name),
        Err(other) => panic!("expected the catch-up to refuse {name}, got {other:?}"),
        Ok(_) => panic!("a Library with two objects named {name} is read by no one"),
    }
}

// CP-3, FM-12: two successors of one head — what a Library created before its
// first head was written at creation can be left with when two first commits
// race on a Storage that mints identifiers. Neither is the Library's more than
// the other, so the Library is refused rather than read through one of them.
#[tokio::test]
async fn two_first_heads_are_refused() {
    refused_with_two_of(&ControlObjectName::head(Generation::FIRST)).await;
}

#[tokio::test]
async fn two_heads_of_one_generation_are_refused() {
    refused_with_two_of(&ControlObjectName::head(generation(1))).await;
}

// CK-11, FM-12: two checkpoints of one head are the same refusal.
#[tokio::test]
async fn two_checkpoints_of_one_head_are_refused() {
    refused_with_two_of(&ControlObjectName::index_snapshot(generation(1))).await;
}

// KL-14: a Keyring replica's name fixes its content, so two of them — two
// devices repairing one position, or a repair beside a copy that did not read
// back valid — are a benign duplicate, and the Library is committed to as
// before.
#[tokio::test]
async fn two_copies_of_one_keyring_replica_are_read_as_one() {
    let store = InMemoryMintingStore::new(PAGE_SIZE);
    committed_once(&store).await;
    let library = Library::read(&store).await;
    let committed = library.record(&store, generation(1)).await;
    let replica = replica_name(committed.keyring(), 0);
    duplicate(&store, &replica.to_string()).await;

    let batch = uploaded(&store, 2).await;
    let outcome = commit_batch(request(
        &store,
        &InMemoryIndex::new(),
        &control_keys(),
        batch,
    ))
    .await
    .unwrap_or_else(|failure| panic!("a duplicated replica is benign: {failure:?}"));
    assert_eq!(outcome.record.generation(), generation(2));
}
