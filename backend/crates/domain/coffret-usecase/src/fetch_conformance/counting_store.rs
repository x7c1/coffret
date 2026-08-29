use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use coffret_model::ObjectRef;

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;

/// A store that counts what a run asks of it, wrapped around the real one.
///
/// One case is about a cost rather than an outcome: a second fetch of a folder
/// nothing happened to must not pull a single Container down again. Nothing the
/// flow returns proves that — an Entry reported skipped could still have been
/// fetched and thrown away — so the case counts the reads instead.
///
/// Another case is about a cost of the opposite kind: reading *one Entry* out of
/// a Pack must not read the Pack. That is a claim about which bytes were asked
/// for and not merely about how many calls were made, so every read is recorded
/// with the range it carried, and the case adds up what was asked of the one
/// object it cares about.
///
/// It wraps whatever store the backend handed the suite, so the case counts real
/// requests against a real provider exactly as it counts them in memory.
pub(super) struct CountingStore<'a> {
    inner: &'a dyn ObjectStore,
    reads: AtomicUsize,
    listings: AtomicUsize,
    /// Every read, in the order it was made: the object and the range asked for,
    /// `None` being a read of the whole object.
    ranges: Mutex<Vec<(ObjectRef, Option<Range<u64>>)>>,
}

impl<'a> CountingStore<'a> {
    /// Starts counting at nothing.
    pub(super) fn around(inner: &'a dyn ObjectStore) -> Self {
        Self {
            inner,
            reads: AtomicUsize::new(0),
            listings: AtomicUsize::new(0),
            ranges: Mutex::new(Vec::new()),
        }
    }

    /// The ranges asked of one object, in the order they were asked for.
    ///
    /// A `None` in the answer is a read of the whole object, which is exactly
    /// what a case about range reads is checking never happened.
    pub(super) fn ranges_of(&self, object: &ObjectRef) -> Vec<Option<Range<u64>>> {
        self.ranges
            .lock()
            .expect("the counting store's own lock is never poisoned")
            .iter()
            .filter(|(read, _)| read == object)
            .map(|(_, range)| range.clone())
            .collect()
    }

    /// How many objects have been read since.
    ///
    /// Control objects included: a run that has a record to replay or a Keyring
    /// to open counts those reads here too, so the number is a Container count
    /// only for a run that needed neither.
    pub(super) fn reads(&self) -> usize {
        self.reads.load(Ordering::Relaxed)
    }

    /// How many listing pages have been asked for since.
    pub(super) fn listings(&self) -> usize {
        self.listings.load(Ordering::Relaxed)
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
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.ranges
            .lock()
            .expect("the counting store's own lock is never poisoned")
            .push((object.clone(), range.clone()));
        self.inner.get(object, range).await
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        self.listings.fetch_add(1, Ordering::Relaxed);
        self.inner.list(page).await
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}
