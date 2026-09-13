//! What a catch-up holds while it looks for a starting point.
//!
//! The walk down the checkpoint candidates passes the records the replay is
//! about to want, and keeping them is what stops each one being fetched twice.
//! How many that is, is not this device's to decide: the checkpoint policy's
//! threshold is a trigger rather than a bound (spec: CK-8), so the stretch
//! between two checkpoints grows for as long as no Snapshot lands — a device
//! that was away, a device that has none of the others' upload luck. So the
//! saving is taken up to a bound and dropped after it (spec: CK-12), and the
//! cases here are about that boundary: what it costs to cross, and that
//! crossing it changes nothing about where the catalog ends up.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Mutex;

use async_trait::async_trait;
use coffret_format::{encode_index_snapshot, encode_journal_record, IndexSnapshotPayload};
use coffret_model::{ControlObjectKind, ControlObjectName, Generation, ObjectRef, SnapshotContent};

use super::catch_up::MAX_HELD_RECORDS;
use super::control_fixtures::{control_keys, once, record_at, store_control, PAGE_SIZE};
use super::{catch_up, ControlKeys};
use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::generations::generation;
use crate::in_memory_index::InMemoryIndex;
use crate::in_memory_store::InMemoryStore;
use crate::index::Index;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;

/// How far past the newest checkpoint the case's Library has committed.
///
/// A few records more than one catch-up holds, so that the walk crosses the
/// bound and the replay has to read something again — and only a few, because
/// what the case counts is the crossing and not the stretch.
const HEADS_PAST_THE_CHECKPOINT: usize = MAX_HELD_RECORDS + 4;

/// A store that answers honestly and remembers which objects were read.
///
/// Nothing here is dishonest: what is being counted is a flow's own reads, and
/// an object read twice is the only way from outside to see that the walk did
/// not keep it.
struct CountingStore<'a> {
    inner: &'a dyn ObjectStore,
    read: Mutex<Vec<ObjectRef>>,
}

impl<'a> CountingStore<'a> {
    fn around(inner: &'a dyn ObjectStore) -> Self {
        Self {
            inner,
            read: Mutex::new(Vec::new()),
        }
    }

    /// How many distinct objects were read more than once.
    fn read_twice(&self) -> usize {
        let read = self.read.lock().expect("no test panics here");
        let mut reads: BTreeMap<&str, usize> = BTreeMap::new();
        for object in read.iter() {
            *reads.entry(object.as_str()).or_default() += 1;
        }
        reads.values().filter(|count| **count > 1).count()
    }
}

#[async_trait]
impl ObjectStore for CountingStore<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<ObjectRef> {
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
        self.read
            .lock()
            .expect("no test panics here")
            .push(object.clone());
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

/// A Library checkpointed at its first head and committed a long way past it.
///
/// One ordinary Snapshot, at generation 0, and a Journal record at every head
/// from there to the newest — the shape a Library takes when the device that
/// would have written the next Snapshot never landed it (spec: CK-8).
async fn checkpointed_at_the_first_head(store: &InMemoryStore, keys: &ControlKeys) {
    for number in 0..=HEADS_PAST_THE_CHECKPOINT as u64 {
        let at = generation(number);
        let record = record_at(at);
        store_control(
            store,
            keys,
            &ControlObjectName::head(at),
            ControlObjectKind::Journal,
            &encode_journal_record(&record).expect("an empty record encodes"),
        )
        .await;

        if at != Generation::FIRST {
            continue;
        }
        let content = SnapshotContent::new(record.checkpoint(), None, Vec::new(), Vec::new())
            .expect("a Library of nothing is one an Index could stand at");
        store_control(
            store,
            keys,
            &ControlObjectName::index_snapshot(at),
            ControlObjectKind::IndexSnapshot,
            &encode_index_snapshot(&IndexSnapshotPayload::ordinary(content))
                .expect("an empty Snapshot encodes"),
        )
        .await;
    }
}

// CK-12: the walk keeps what the replay will want, and only up to the bound. A
// stretch longer than that is walked all the same and replayed all the same —
// the records past the bound are simply read a second time instead of held, so
// the memory one catch-up costs is this device's number rather than the
// Library's length.
//
// The second read is what makes the bound visible from outside: a record the
// walk kept is never fetched again, so the count of objects read twice is
// exactly the count of records the walk declined to hold.
#[tokio::test]
async fn holds_no_more_than_it_will_replay() {
    let store = InMemoryStore::new(PAGE_SIZE);
    let keys = control_keys();
    checkpointed_at_the_first_head(&store, &keys).await;
    let counting = CountingStore::around(&store);

    let index = InMemoryIndex::new();
    catch_up(&counting, &index, &keys, &once())
        .await
        .expect("a Library committed past its checkpoint is one to catch up with");

    // Every head above the checkpoint carries a record the walk passes and the
    // replay applies; the ones past the bound are the ones read twice.
    let walked = HEADS_PAST_THE_CHECKPOINT;
    assert_eq!(
        counting.read_twice(),
        walked - MAX_HELD_RECORDS,
        "a walk of {walked} records must hold {MAX_HELD_RECORDS} of them and read the rest again",
    );

    // And the bound changes nothing about the answer: the catalog stands at the
    // head, started from the one checkpoint there was (spec: CK-9).
    let standing = index
        .snapshot()
        .await
        .expect("a caught-up catalog stands somewhere");
    assert_eq!(
        standing.adopted_from(),
        Some(&ControlObjectName::index_snapshot(Generation::FIRST)),
        "the only checkpoint is what the catalog was started from",
    );
    assert_eq!(
        standing.checkpoint().head_generation(),
        generation(walked as u64),
        "and every record after it was replayed, leaving the catalog at the head",
    );
}
