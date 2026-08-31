use std::ops::Range;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use coffret_model::ObjectRef;
use coffret_usecase::{
    ByteStream, CommitSlot, ObjectPage, ObjectStore, PageToken, Result as StoreResult,
};

/// A store that records what was read of it, wrapped around the real one.
///
/// One case is about a cost rather than an outcome: two browsers asking for one
/// Entry at the same moment must fetch it once. Nothing a response carries
/// proves that — both requests answer with the same bytes either way — so the
/// case counts the reads instead, and a range read is what placing an Entry out
/// of a Container costs (spec: PK-16).
pub struct CountingStore {
    inner: Arc<dyn ObjectStore>,
    reads: Mutex<Vec<Option<Range<u64>>>>,
}

impl CountingStore {
    /// Starts counting at nothing.
    pub fn around(inner: Arc<dyn ObjectStore>) -> Self {
        Self {
            inner,
            reads: Mutex::new(Vec::new()),
        }
    }

    /// Forgets what has been read so far.
    ///
    /// Called once the fixture is built, so a case counts its own requests
    /// rather than the catch-up that got the device to the Library's head.
    pub fn forget(&self) {
        self.locked().clear();
    }

    /// How many reads asked for part of an object rather than all of it.
    pub fn ranged_reads(&self) -> usize {
        self.locked().iter().filter(|range| range.is_some()).count()
    }

    fn locked(&self) -> std::sync::MutexGuard<'_, Vec<Option<Range<u64>>>> {
        self.reads
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl ObjectStore for CountingStore {
    async fn put(&self, name: &str, body: ByteStream) -> StoreResult<ObjectRef> {
        self.inner.put(name, body).await
    }

    async fn reserve_create(&self, name: &str) -> StoreResult<CommitSlot> {
        self.inner.reserve_create(name).await
    }

    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> StoreResult<ObjectRef> {
        self.inner.put_if_absent(slot, body).await
    }

    fn object_at(&self, slot: &CommitSlot) -> StoreResult<ObjectRef> {
        self.inner.object_at(slot)
    }

    async fn get(&self, object: &ObjectRef, range: Option<Range<u64>>) -> StoreResult<ByteStream> {
        self.locked().push(range.clone());
        self.inner.get(object, range).await
    }

    async fn list(&self, page: Option<&PageToken>) -> StoreResult<ObjectPage> {
        self.inner.list(page).await
    }

    async fn trash(&self, object: &ObjectRef) -> StoreResult<()> {
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> StoreResult<()> {
        self.inner.purge(object).await
    }
}
