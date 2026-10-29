//! The prune's contract, as tests (spec: CK-4, CK-5, CK-6, CK-8, KL-18).
//!
//! A Library is built by real commits — its first head and a record per batch,
//! each with its Keyring set — so the replica set the gate reads is the one a
//! commit wrote, and the records a prune removes are the ones a catch-up would
//! otherwise replay. What a case asserts about Storage is read off its listing
//! afterwards, by name, because what `prune` promises is which objects are
//! still there: every one but the eligible Journal records and the replicas of
//! the Keyring generations older than the one the Snapshot records.

use std::collections::BTreeSet;
use std::ops::Range;

use async_trait::async_trait;
use coffret_format::{
    encode_control_object, encode_index_snapshot, ControlEncodeRequest, IndexSnapshotPayload,
    Purpose, PurposeKey,
};
use coffret_model::{
    ContainerKind, ControlObjectKind, ControlObjectName, Generation, IndexCheckpoint,
    KeyringCommitment, MasterKeyEpoch, ObjectRef, ReplicaPosition, SnapshotContent,
};

use super::{prune_journal, PruneError, PruneOutcome, PruneRefusal, PruneRequest};
use crate::byte_stream::ByteStream;
use crate::commit::{commit_batch, write_first_head, CommitPolicy, CommitRequest, PreparedBatch};
use crate::commit_conformance::faulty_store::FaultyStore;
use crate::commit_conformance::fixtures::{container_id, master_key, prepared};
use crate::commit_slot::CommitSlot;
use crate::error::{Error, Result};
use crate::generations::generation;
use crate::in_memory_index::InMemoryIndex;
use crate::in_memory_store::InMemoryStore;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::missing::Missing;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::uploaded_object::UploadedObject;

/// The keys of the Library's first epoch, under the suite's Master Key.
fn keys() -> LibraryKeys {
    LibraryKeys::derive(&master_key(), MasterKeyEpoch::FIRST)
}

/// Two replicas, so a case can lose one and still hold a set; and a threshold
/// no case reaches, so a Snapshot is on Storage only where a case asked for one.
fn policy() -> CommitPolicy {
    CommitPolicy::default()
        .with_replica_count(2)
        .with_checkpoint_threshold(1_000)
}

/// The prune of the Library in `store` through the device whose catalog is
/// `index`.
fn request<'a>(
    store: &'a dyn ObjectStore,
    index: &'a dyn Index,
    keys: &'a LibraryKeys,
) -> PruneRequest<'a> {
    PruneRequest::new(store, index, keys).with_policy(policy())
}

/// Commits one batch adding a Container of one file, under `policy`.
async fn commit(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &LibraryKeys,
    seed: u8,
    policy: CommitPolicy,
) -> Generation {
    let path = format!("albums/{seed}.jpg");
    let batch = PreparedBatch::adding(vec![prepared(seed, ContainerKind::OneFile, &[&path])]);
    commit_batch(CommitRequest::new(store, index, keys.control(), batch).with_policy(policy))
        .await
        .unwrap_or_else(|error| panic!("committing batch {seed} must succeed: {error}"))
        .record
        .generation()
}

/// A Library holding its first head and `commits` more records, one per
/// batch, committed through `index`: heads `0` to `commits`, and no Snapshot.
///
/// Each batch's Container is put on Storage too, under its own name, so that a
/// case can see a prune leave it where it was (spec: CK-6).
async fn library(store: &InMemoryStore, index: &dyn Index, keys: &LibraryKeys, commits: u8) {
    write_first_head(store, keys.control(), &policy(), None)
        .await
        .expect("creating a Library in an empty store must succeed")
        .expect("an empty store holds no head to find instead");
    for seed in 1..=commits {
        put(store, &container_id(seed).object_name(), b"ciphertext").await;
        commit(store, index, keys, seed, policy()).await;
    }
}

/// Stores `bytes` under `name`, unconditionally.
async fn put(store: &dyn ObjectStore, name: &str, bytes: &[u8]) {
    store
        .put(name, ByteStream::from(bytes.to_vec()))
        .await
        .expect("storing an object must succeed");
}

/// Every name the listing holds.
async fn names(store: &dyn ObjectStore) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut token = None;
    loop {
        let page = store
            .list(token.as_ref())
            .await
            .expect("listing must succeed");
        names.extend(page.objects.into_iter().map(|object| object.name));
        token = page.next;
        if token.is_none() {
            return names;
        }
    }
}

/// The names of the heads at `generations`.
fn heads(generations: Range<u64>) -> BTreeSet<String> {
    generations
        .map(|number| ControlObjectName::head(generation(number)).to_string())
        .collect()
}

/// The generations in `range`.
fn generations(range: Range<u64>) -> Vec<Generation> {
    range.map(generation).collect()
}

/// The names in `names` of the Keyring replicas whose generation is below
/// `bound`.
fn replicas_below(names: &BTreeSet<String>, bound: Generation) -> BTreeSet<String> {
    replicas_where(names, |generation| generation < bound)
}

/// The names in `names` of the Keyring replicas of `generation`.
fn replicas_of(names: &BTreeSet<String>, generation: Generation) -> BTreeSet<String> {
    replicas_where(names, |found| found == generation)
}

/// The names in `names` of the Keyring replicas whose generation `keep` says
/// yes to.
fn replicas_where(names: &BTreeSet<String>, keep: impl Fn(Generation) -> bool) -> BTreeSet<String> {
    names
        .iter()
        .filter(|name| {
            matches!(
                ControlObjectName::parse(name),
                Ok(ControlObjectName::KeyringReplica { generation, .. }) if keep(generation)
            )
        })
        .cloned()
        .collect()
}

/// The names of the replicas an outcome says it trashed.
fn trashed(outcome: &PruneOutcome) -> BTreeSet<String> {
    outcome
        .removed_replicas
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// The Keyring generation the head a catalog stands at selects.
async fn keyring_generation(index: &dyn Index) -> Generation {
    checkpoint(index).await.keyring().generation()
}

/// The checkpoint a catalog stands at.
async fn checkpoint(index: &dyn Index) -> IndexCheckpoint {
    index
        .checkpoint()
        .await
        .expect("reading the checkpoint must succeed")
        .expect("the catalog stands at a head")
}

/// How many current Entries a catalog holds.
async fn entries(index: &dyn Index) -> usize {
    index
        .entries_under(None)
        .await
        .expect("reading the catalog must succeed")
        .len()
}

/// Runs a prune that has to succeed.
async fn pruned(store: &dyn ObjectStore, index: &dyn Index, keys: &LibraryKeys) -> PruneOutcome {
    prune_journal(request(store, index, keys))
        .await
        .unwrap_or_else(|error| panic!("the prune must succeed: {error}"))
}

/// Runs a prune that has to be refused, and hands back why.
async fn refused(store: &dyn ObjectStore, index: &dyn Index, keys: &LibraryKeys) -> PruneRefusal {
    match prune_journal(request(store, index, keys)).await {
        Err(PruneError::Refused(refusal)) => *refusal,
        other => panic!("expected the prune to be refused, got {other:?}"),
    }
}

/// Seals `content` as an ordinary Snapshot and stores it at `name`, as a
/// writer other than the flow under test would.
async fn store_snapshot(
    store: &dyn ObjectStore,
    name: &ControlObjectName,
    content: SnapshotContent,
) {
    let payload = encode_index_snapshot(&IndexSnapshotPayload::ordinary(content))
        .expect("a Snapshot of a real catalog encodes");
    let object = encode_control_object(&ControlEncodeRequest::new(
        name,
        ControlObjectKind::IndexSnapshot,
        &PurposeKey::derive(&master_key(), Purpose::ControlIndexSnapshot),
        &payload,
    ))
    .expect("sealing a Snapshot under a real key must succeed");
    put(store, &name.to_string(), object.bytes()).await;
}

// CK-4, CK-6, CK-8, KL-18: every record at or before what the Snapshot applies
// goes, but the head; the Snapshot of the head is written first, since none was
// there; then every replica of every Keyring generation older than the one the
// Snapshot records goes too. Nothing else on Storage moves — not the
// generation the head selects, no Container. The records and the replicas are
// trashed rather than purged, so a person can still get them back.
#[tokio::test]
async fn every_record_the_snapshot_covers_goes_but_the_head() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&store, &index, &keys, 3).await;
    let before = names(&store).await;
    assert!(
        !before.contains("idx-3.cfrt"),
        "no Snapshot is due yet: {before:?}"
    );

    let outcome = pruned(&store, &index, &keys).await;

    assert_eq!(outcome.head, generation(3));
    assert_eq!(outcome.snapshot.to_string(), "idx-3.cfrt");
    assert!(outcome.wrote_snapshot, "no Snapshot of the head was there");
    assert_eq!(outcome.covered_through, generation(3));
    assert_eq!(outcome.removed, generations(0..3));
    assert_eq!(
        outcome.removed_range(),
        Some((generation(0), generation(2)))
    );
    assert!(outcome.already_gone.is_empty(), "{outcome:?}");
    assert!(outcome.kept.is_empty(), "{outcome:?}");
    assert!(outcome.unremoved.is_empty(), "{outcome:?}");
    let selected = keyring_generation(&index).await;
    let superseded = replicas_below(&before, selected);
    assert!(!superseded.is_empty(), "every commit wrote a generation");
    assert_eq!(trashed(&outcome), superseded);
    assert!(outcome.unremoved_replicas.is_empty(), "{outcome:?}");

    let mut expected: BTreeSet<String> = before
        .difference(&heads(0..3))
        .cloned()
        .collect::<BTreeSet<_>>()
        .difference(&superseded)
        .cloned()
        .collect();
    expected.insert("idx-3.cfrt".to_owned());
    let after = names(&store).await;
    assert_eq!(
        after, expected,
        "the records before the head and the older generations left, the Snapshot arrived, \
         and nothing else changed",
    );
    assert_eq!(
        replicas_where(&after, |_| true),
        replicas_of(&before, selected),
        "the generation the Snapshot records and the head selects is all of the Keyring left",
    );
    for removed in heads(0..3).into_iter().chain(superseded) {
        store
            .get(&ObjectRef::new(&removed), None)
            .await
            .unwrap_or_else(|error| panic!("{removed} is in the trash, not purged: {error}"));
    }
}

// CK-8: a Snapshot of the head another run already wrote is the one the prune
// rests on — read, not uploaded again.
#[tokio::test]
async fn a_snapshot_already_written_is_the_one_rested_on() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&store, &index, &keys, 2).await;
    put(&store, &container_id(3).object_name(), b"ciphertext").await;
    commit(
        &store,
        &index,
        &keys,
        3,
        policy().with_checkpoint_threshold(0),
    )
    .await;
    let written = names(&store).await;
    assert!(written.contains("idx-3.cfrt"), "the commit checkpointed");

    let outcome = pruned(&store, &index, &keys).await;

    assert_eq!(outcome.snapshot.to_string(), "idx-3.cfrt");
    assert!(!outcome.wrote_snapshot, "the Snapshot was already there");
    assert_eq!(outcome.removed, generations(0..3));
    let superseded = replicas_below(&written, keyring_generation(&index).await);
    assert_eq!(trashed(&outcome), superseded);
    assert_eq!(
        names(&store).await,
        written
            .difference(&heads(0..3))
            .cloned()
            .collect::<BTreeSet<_>>()
            .difference(&superseded)
            .cloned()
            .collect(),
        "no second Snapshot of the head, and nothing but the records and the older generations \
         removed",
    );
}

// CK-4, CK-6: an older Snapshot is not a Journal record, and stays where it is
// once a newer one covers it: no rule says when a superseded Snapshot may go.
#[tokio::test]
async fn an_older_snapshot_stays() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&store, &index, &keys, 1).await;
    put(&store, &container_id(2).object_name(), b"ciphertext").await;
    commit(
        &store,
        &index,
        &keys,
        2,
        policy().with_checkpoint_threshold(0),
    )
    .await;
    put(&store, &container_id(3).object_name(), b"ciphertext").await;
    commit(&store, &index, &keys, 3, policy()).await;

    let outcome = pruned(&store, &index, &keys).await;

    assert_eq!(outcome.removed, generations(0..3));
    let after = names(&store).await;
    assert!(after.contains("idx-2.cfrt"), "{after:?}");
    assert!(after.contains("idx-3.cfrt"), "{after:?}");
}

// CK-6: a head position holding an activation Snapshot is not a Journal
// record, so a prune keeps it, whatever generation it stands at. Only its
// header is read to tell — the kind byte, which no key is needed for.
#[tokio::test]
async fn an_activation_snapshot_in_a_head_position_stays() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&store, &index, &keys, 3).await;
    // Over the record at head 1, which this catalog has already applied, so no
    // catch-up of it reads the object again.
    let name = ControlObjectName::head(generation(1));
    let content = index.snapshot().await.expect("the catalog snapshots");
    let payload = encode_index_snapshot(&IndexSnapshotPayload::ordinary(content))
        .expect("a Snapshot of a real catalog encodes");
    let object = encode_control_object(&ControlEncodeRequest::new(
        &name,
        ControlObjectKind::ActivationSnapshot,
        &PurposeKey::derive(&master_key(), Purpose::ControlActivationSnapshot),
        &payload,
    ))
    .expect("sealing under a real key must succeed");
    put(&store, &name.to_string(), object.bytes()).await;

    let outcome = pruned(&store, &index, &keys).await;

    assert_eq!(outcome.kept, vec![generation(1)]);
    assert_eq!(outcome.removed, vec![generation(0), generation(2)]);
    assert!(names(&store).await.contains("head-1.cfrt"));
}

// CK-5, CK-3, KL-18: a Snapshot of the head that records another Keyring tuple
// than the head committed would, once the records were gone, prove the wrong
// replica set selected. Nothing is removed — no record, and no older Keyring
// generation either.
#[tokio::test]
async fn a_snapshot_recording_another_keyring_tuple_is_refused() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&store, &index, &keys, 3).await;
    let (held, _, containers, entries) = index
        .snapshot()
        .await
        .expect("the catalog snapshots")
        .into_parts();
    let committed = held.keyring().clone();
    let other = KeyringCommitment::new(
        committed.generation(),
        committed.replica_count() + 1,
        committed.set_digest(),
    )
    .expect("a real digest under another count is a commitment");
    let wrong = IndexCheckpoint::at_head(
        held.master_key_epoch(),
        held.head_generation(),
        held.next_commit_slot().map(str::to_owned),
        other.clone(),
    );
    let name = ControlObjectName::index_snapshot(generation(3));
    store_snapshot(
        &store,
        &name,
        SnapshotContent::new(wrong, None, containers, entries)
            .expect("the catalog's own content holds together"),
    )
    .await;
    let before = names(&store).await;

    match refused(&store, &index, &keys).await {
        PruneRefusal::KeyringNotCommitted {
            snapshot,
            recorded,
            committed: said,
            ..
        } => {
            assert_eq!(snapshot, name);
            assert_eq!(recorded, other);
            assert_eq!(said, committed);
        }
        other => panic!("expected the tuple to be refused, got {other:?}"),
    }
    assert_eq!(names(&store).await, before, "nothing was removed");
}

// CK-5, KL-2, KL-18: a committed set missing a replica is not complete, and
// once the records were gone its replicas would be the envelopes' only
// carriers. Nothing is removed — no record and no older Keyring generation —
// but the Snapshot was written first, and stands (spec: CK-8).
#[tokio::test]
async fn an_incomplete_keyring_set_is_refused() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&store, &index, &keys, 3).await;
    let committed = checkpoint(&index).await.keyring().clone();
    let lost = ControlObjectName::keyring_replica(
        committed.generation(),
        committed.set_digest(),
        ReplicaPosition::new(1, committed.replica_count()).expect("position 1 of 2"),
    )
    .expect("a committed tuple names its replicas");
    store
        .purge(&ObjectRef::new(lost.to_string()))
        .await
        .expect("purging must succeed");
    let before = names(&store).await;

    match refused(&store, &index, &keys).await {
        PruneRefusal::KeyringIncomplete { snapshot, short } => {
            assert_eq!(snapshot.to_string(), "idx-3.cfrt");
            assert_eq!(short.generation(), committed.generation());
            assert_eq!(short.lost(), 1);
            assert_eq!(short.unfetched(), 0);
        }
        other => panic!("expected the set to be refused, got {other:?}"),
    }
    let mut expected = before;
    expected.insert("idx-3.cfrt".to_owned());
    assert_eq!(
        names(&store).await,
        expected,
        "the Snapshot was written, and nothing was removed",
    );
}

// CK-5: a replica Storage will not hand over is not known lost, and is not
// known present either: the set is not shown complete, so nothing goes.
#[tokio::test]
async fn a_replica_storage_will_not_hand_over_is_refused_too() {
    let keys = keys();
    let inner = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&inner, &index, &keys, 2).await;
    let committed = checkpoint(&index).await.keyring().clone();
    let hidden = ControlObjectName::keyring_replica(
        committed.generation(),
        committed.set_digest(),
        ReplicaPosition::new(0, committed.replica_count()).expect("position 0 of 2"),
    )
    .expect("a committed tuple names its replicas");
    let store = FaultyStore::hiding(&inner, &ObjectRef::new(hidden.to_string()));

    match refused(&store, &index, &keys).await {
        PruneRefusal::KeyringIncomplete { short, .. } => {
            assert_eq!(short.lost(), 0);
            assert_eq!(short.unfetched(), 1);
            assert!(!short.established());
        }
        other => panic!("expected the set to be refused, got {other:?}"),
    }
    assert!(names(&inner).await.is_superset(&heads(0..3)));
}

// FM-13: a Library that has committed nothing has no head to checkpoint.
#[tokio::test]
async fn a_library_with_no_head_has_nothing_to_prune() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();

    assert!(matches!(
        refused(&store, &index, &keys).await,
        PruneRefusal::NothingCommitted
    ));
    assert!(names(&store).await.is_empty());
}

// CK-4, KL-18: a prune straight after another removes nothing and succeeds —
// the head is all that is left of the Journal, and it stays, and the
// generation it selects is all that is left of the Keyring.
#[tokio::test]
async fn a_second_prune_removes_nothing() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&store, &index, &keys, 3).await;
    pruned(&store, &index, &keys).await;
    let before = names(&store).await;

    let again = pruned(&store, &index, &keys).await;

    assert!(!again.wrote_snapshot);
    assert!(again.removed.is_empty(), "{again:?}");
    assert_eq!(again.removed_range(), None);
    assert!(again.already_gone.is_empty(), "{again:?}");
    assert!(again.removed_replicas.is_empty(), "{again:?}");
    assert!(again.unremoved_replicas.is_empty(), "{again:?}");
    assert_eq!(names(&store).await, before);
}

// CK-6, KL-18: a record Storage will not trash is reported and the run goes
// on; what it removed stays removed, and the next prune finishes the job. While
// a record is left in place no Keyring generation goes, since that record
// still selects the one it committed; the prune that removes it takes them.
#[tokio::test]
async fn a_record_storage_will_not_trash_is_reported_and_left_for_the_next_prune() {
    let keys = keys();
    let inner = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&inner, &index, &keys, 3).await;
    let before = names(&inner).await;
    let superseded = replicas_below(&before, keyring_generation(&index).await);

    let refusing = FaultyStore::refusing_to_trash(&inner);
    let outcome = pruned(&refusing, &index, &keys).await;
    assert!(outcome.removed.is_empty(), "{outcome:?}");
    assert_eq!(
        outcome
            .unremoved
            .iter()
            .map(|unremoved| unremoved.generation)
            .collect::<Vec<_>>(),
        generations(0..3),
    );
    assert!(names(&inner).await.is_superset(&heads(0..4)));

    let hiding = FaultyStore::hiding(&inner, &ObjectRef::new("head-1.cfrt"));
    let outcome = pruned(&hiding, &index, &keys).await;
    assert_eq!(outcome.removed, vec![generation(0), generation(2)]);
    assert_eq!(outcome.unremoved.len(), 1, "{outcome:?}");
    assert_eq!(outcome.unremoved[0].generation, generation(1));
    assert!(outcome.removed_replicas.is_empty(), "{outcome:?}");
    assert!(names(&inner).await.is_superset(&superseded));

    let outcome = pruned(&inner, &index, &keys).await;
    assert_eq!(outcome.removed, vec![generation(1)]);
    assert_eq!(trashed(&outcome), superseded);
    let after = names(&inner).await;
    assert!(after.is_disjoint(&heads(0..3)));
    assert!(after.is_disjoint(&superseded));
}

/// A store over another, on which one object is gone by the time it is read:
/// another device pruned it between the listing and the read.
struct Vanished<'a> {
    inner: &'a dyn ObjectStore,
    gone: String,
}

#[async_trait]
impl ObjectStore for Vanished<'_> {
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
        if object.as_str() == self.gone {
            return Err(Error::NotFound {
                missing: Missing::Object(self.gone.clone()),
            });
        }
        self.inner.get(object, range).await
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        self.inner.list(page).await
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}

// CK-6: a record another device pruned first is where this run was taking it,
// and is no failure.
#[tokio::test]
async fn a_record_already_gone_is_not_a_failure() {
    let keys = keys();
    let inner = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&inner, &index, &keys, 3).await;
    let store = Vanished {
        inner: &inner,
        gone: "head-1.cfrt".to_owned(),
    };

    let outcome = pruned(&store, &index, &keys).await;

    assert_eq!(outcome.removed, vec![generation(0), generation(2)]);
    assert_eq!(outcome.already_gone, vec![generation(1)]);
    assert!(outcome.unremoved.is_empty(), "{outcome:?}");
}

// CK-2, CP-2: the head a prune keeps carries the slot the next commit goes
// into, so a commit after it lands as any other does.
#[tokio::test]
async fn a_commit_after_a_prune_lands() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&store, &index, &keys, 3).await;
    pruned(&store, &index, &keys).await;

    let landed = commit(&store, &index, &keys, 4, policy()).await;

    assert_eq!(landed, generation(4));
    assert_eq!(entries(&index).await, 4);
}

// CK-9: a device whose catalog stands before the Snapshot finds the records it
// would have replayed gone, and starts from the Snapshot instead: the same
// head, the same Catalog — and it commits from there.
//
// It catches up before it prepares anything, as every flow does: a batch
// prepared against a head the prune has since removed has no records left to
// be rebased over, and is refused as the gap it is (spec: CP-4).
#[tokio::test]
async fn a_device_behind_the_snapshot_catches_up_from_it() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let pruning = InMemoryIndex::new();
    library(&store, &pruning, &keys, 1).await;
    let behind = InMemoryIndex::new();
    crate::commit::catch_up(&store, &behind, keys.control(), &policy().retry)
        .await
        .expect("the second device catches up");
    assert_eq!(checkpoint(&behind).await.head_generation(), generation(1));
    for seed in 2..=3 {
        put(&store, &container_id(seed).object_name(), b"ciphertext").await;
        commit(&store, &pruning, &keys, seed, policy()).await;
    }
    pruned(&store, &pruning, &keys).await;

    crate::commit::catch_up(&store, &behind, keys.control(), &policy().retry)
        .await
        .expect("the device behind catches up past the pruned records");

    assert_eq!(checkpoint(&behind).await.head_generation(), generation(3));
    assert_eq!(entries(&behind).await, 3);
    assert_eq!(
        behind
            .snapshot()
            .await
            .expect("the catalog snapshots")
            .adopted_from()
            .map(ToString::to_string)
            .as_deref(),
        Some("idx-3.cfrt"),
        "the catalog came from the Snapshot, not from records that are gone",
    );
    let landed = commit(&store, &behind, &keys, 4, policy()).await;
    assert_eq!(landed, generation(4));
    assert_eq!(entries(&behind).await, 4);
}

// CK-4, CK-6, KL-4, KL-18, RV-1: the first head is gone, and a device joining asks for
// any head or Snapshot rather than for it by name — there are both — and
// catches up from nothing to the head the Library stands at. With the records
// that selected it gone, the Snapshot is what still says which Keyring set the
// Library committed, and the joined catalog holds that tuple.
#[tokio::test]
async fn a_device_joining_after_a_prune_finds_the_library() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&store, &index, &keys, 3).await;
    pruned(&store, &index, &keys).await;

    let listed = names(&store).await;
    assert!(!listed.contains("head-0.cfrt"), "{listed:?}");
    assert!(
        listed.iter().any(|name| matches!(
            ControlObjectName::parse(name),
            Ok(ControlObjectName::Head { .. } | ControlObjectName::IndexSnapshot { .. })
        )),
        "{listed:?}",
    );

    let joined = InMemoryIndex::new();
    crate::commit::catch_up(&store, &joined, keys.control(), &policy().retry)
        .await
        .expect("a device holding nothing catches up");
    assert_eq!(checkpoint(&joined).await.head_generation(), generation(3));
    assert_eq!(entries(&joined).await, 3);
    assert_eq!(
        checkpoint(&joined).await.keyring(),
        checkpoint(&index).await.keyring(),
        "the committed tuple outlives the records that selected it",
    );
}

/// A store over another that will not trash one object, and trashes every
/// other: credentials that may not delete it, or a provider that keeps failing
/// on it.
struct RefusingOne<'a> {
    inner: &'a dyn ObjectStore,
    refused: String,
}

#[async_trait]
impl ObjectStore for RefusingOne<'_> {
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
        self.inner.list(page).await
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        if object.as_str() == self.refused {
            return Err(FaultyStore::trash_refusal());
        }
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}

// KL-18: a superseded replica Storage will not trash is reported and the run
// goes on to the rest; what it trashed stays trashed, and the next prune takes
// the one left.
#[tokio::test]
async fn a_replica_storage_will_not_trash_is_reported_and_left_for_the_next_prune() {
    let keys = keys();
    let inner = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&inner, &index, &keys, 3).await;
    let superseded = replicas_below(&names(&inner).await, keyring_generation(&index).await);
    let refused = superseded
        .first()
        .expect("every commit wrote a generation")
        .clone();
    let store = RefusingOne {
        inner: &inner,
        refused: refused.clone(),
    };

    let outcome = pruned(&store, &index, &keys).await;

    assert_eq!(outcome.removed, generations(0..3), "the records went first");
    assert_eq!(outcome.unremoved_replicas.len(), 1, "{outcome:?}");
    assert_eq!(outcome.unremoved_replicas[0].replica.to_string(), refused);
    let rest: BTreeSet<String> = superseded
        .iter()
        .filter(|name| **name != refused)
        .cloned()
        .collect();
    assert_eq!(trashed(&outcome), rest);
    let after = names(&inner).await;
    assert!(after.contains(&refused), "{after:?}");
    assert!(after.is_disjoint(&rest), "{after:?}");

    let again = pruned(&inner, &index, &keys).await;
    assert_eq!(trashed(&again), BTreeSet::from([refused]));
    assert!(again.unremoved_replicas.is_empty(), "{again:?}");
}

// KL-12, KL-14, KL-18: a name carries a generation and not a commitment, so a
// candidate that lost its commit slot at a superseded generation's number goes
// with the committed set beside it, to the trash. One at the number the head
// selects stays, as that generation does.
#[tokio::test]
async fn a_lost_candidate_goes_with_its_superseded_generation() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&store, &index, &keys, 3).await;
    let selected = keyring_generation(&index).await;
    let older = replicas_below(&names(&store).await, selected)
        .first()
        .map(|name| {
            ControlObjectName::parse(name)
                .expect("a listed replica's name parses")
                .generation()
        })
        .expect("every commit wrote a generation");
    let lost_at = |at: Generation| -> BTreeSet<String> {
        (0..2)
            .map(|position| {
                ControlObjectName::keyring_replica(
                    at,
                    "ab12",
                    ReplicaPosition::new(position, 2).expect("a position of two"),
                )
                .expect("a hex digest names a replica")
                .to_string()
            })
            .collect()
    };
    let superseded = lost_at(older);
    let current = lost_at(selected);
    for name in superseded.iter().chain(&current) {
        put(&store, name, b"a candidate that lost its commit slot").await;
    }

    let outcome = pruned(&store, &index, &keys).await;

    assert!(trashed(&outcome).is_superset(&superseded), "{outcome:?}");
    assert!(trashed(&outcome).is_disjoint(&current), "{outcome:?}");
    let after = names(&store).await;
    assert!(after.is_disjoint(&superseded), "{after:?}");
    assert!(after.is_superset(&current), "{after:?}");
    for removed in superseded {
        store
            .get(&ObjectRef::new(&removed), None)
            .await
            .unwrap_or_else(|error| panic!("{removed} is in the trash, not purged: {error}"));
    }
}

// KL-18: only generations older than the Snapshot's go. A set named past the
// head — a candidate a commit is writing right now, or one a prepared Master
// Key epoch wrote ahead of its activation — is no superseded generation of
// this epoch, and stays.
#[tokio::test]
async fn a_set_past_the_head_stays() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&store, &index, &keys, 3).await;
    let committed = checkpoint(&index).await.keyring().clone();
    let own = replicas_of(&names(&store).await, committed.generation());
    let ahead = committed
        .generation()
        .next()
        .expect("a small generation has a successor");
    let candidate: BTreeSet<String> = (0..2)
        .map(|position| {
            ControlObjectName::keyring_replica(
                ahead,
                committed.set_digest(),
                ReplicaPosition::new(position, 2).expect("a position of two"),
            )
            .expect("a real digest names a replica")
            .to_string()
        })
        .collect();
    for name in &candidate {
        put(&store, name, b"sealed under the epoch it was prepared for").await;
    }

    let outcome = pruned(&store, &index, &keys).await;

    assert!(trashed(&outcome).is_disjoint(&candidate), "{outcome:?}");
    let after = names(&store).await;
    assert!(after.is_superset(&candidate), "{after:?}");
    assert!(
        !own.is_empty() && after.is_superset(&own),
        "and the head's own generation stays"
    );
}

// KL-10, KL-18: past a Master Key rotation the older generation numbers
// include the previous epochs' generations, which no name tells apart from the
// current epoch's. Rotation is not implemented, and a Library standing in a
// later epoch keeps every Keyring generation: another epoch's is never
// trashed, at the price of the current epoch's waiting.
#[tokio::test]
async fn a_library_past_its_first_epoch_keeps_every_generation() {
    let later = MasterKeyEpoch::FIRST
        .next()
        .expect("the first epoch has a successor");
    let keys = LibraryKeys::derive(&master_key(), later);
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&store, &index, &keys, 3).await;
    assert_eq!(checkpoint(&index).await.master_key_epoch(), later);
    let before = names(&store).await;

    let outcome = pruned(&store, &index, &keys).await;

    assert_eq!(outcome.removed, generations(0..3), "the records still go");
    assert!(outcome.removed_replicas.is_empty(), "{outcome:?}");
    assert!(
        names(&store)
            .await
            .is_superset(&replicas_where(&before, |_| true)),
        "every Keyring replica stays",
    );
}

// KL-13, KL-18: repair rewrites only the generation the device's caught-up
// head selects. A replica missing from a superseded generation — one lost
// before any prune, or every one a prune trashed — is not a degraded set, and
// the next commit writes none of it back.
#[tokio::test]
async fn repair_never_rewrites_a_superseded_generation() {
    let keys = keys();
    let store = InMemoryStore::new(8);
    let index = InMemoryIndex::new();
    library(&store, &index, &keys, 3).await;
    let before = names(&store).await;
    let selected = keyring_generation(&index).await;
    let lost = replicas_below(&before, selected)
        .first()
        .expect("every commit wrote a generation")
        .clone();
    store
        .purge(&ObjectRef::new(&lost))
        .await
        .expect("purging must succeed");

    put(&store, &container_id(4).object_name(), b"ciphertext").await;
    let outcome = commit_batch(
        CommitRequest::new(
            &store,
            &index,
            keys.control(),
            PreparedBatch::adding(vec![prepared(4, ContainerKind::OneFile, &["albums/4.jpg"])]),
        )
        .with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("committing over a superseded loss must succeed: {error}"));
    assert!(outcome.repairs.is_empty(), "{:?}", outcome.repairs);
    assert!(
        !names(&store).await.contains(&lost),
        "nothing wrote it back"
    );

    let pruning = pruned(&store, &index, &keys).await;
    let disposed = trashed(&pruning);
    assert!(!disposed.is_empty(), "{pruning:?}");

    let landed = commit(&store, &index, &keys, 5, policy()).await;
    assert_eq!(landed, generation(5));
    let after = names(&store).await;
    assert!(
        after.is_disjoint(&disposed),
        "nothing wrote a disposed replica back"
    );
    assert!(!after.contains(&lost), "{after:?}");
}
