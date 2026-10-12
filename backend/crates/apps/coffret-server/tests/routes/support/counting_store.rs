use std::ops::Range;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use coffret_model::ObjectRef;
use coffret_usecase::{
    ByteStream, CommitSlot, ObjectPage, ObjectStore, PageToken, Result as StoreResult,
    UploadedObject,
};

use super::Asked;

/// A store that records what was read of it, wrapped around the real one.
///
/// Some cases are about a cost rather than an outcome: two browsers asking for
/// one Entry at the same moment must fetch it once, and a second page out of a
/// book must not ask Storage again for what the first already learned. Nothing
/// a response carries proves either — the bytes are the same whatever was read
/// — so those cases count the calls instead: the listing, a whole object (which
/// is what a Keyring replica read is), and the ranges of a Container, which
/// are its front and the parcels an Entry overlaps (spec: PK-16).
pub struct CountingStore {
    inner: Arc<dyn ObjectStore>,
    reads: Mutex<Vec<Asked>>,
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

    /// How many reads asked for a range of an object rather than all of it.
    pub fn ranged_reads(&self) -> usize {
        self.locked()
            .iter()
            .filter(|asked| matches!(asked, Asked::Range(_)))
            .count()
    }

    /// How many reads asked for a range longer than an object's front, which
    /// is what a parcel read is (spec: PK-16, PK-19).
    pub fn parcel_reads(&self) -> usize {
        self.locked()
            .iter()
            .filter(
                |asked| matches!(asked, Asked::Range(range) if range.end - range.start > 64 * 1024),
            )
            .count()
    }

    /// Every call counted so far, in the order it was made.
    pub fn asked(&self) -> Vec<Asked> {
        self.locked().clone()
    }

    fn locked(&self) -> std::sync::MutexGuard<'_, Vec<Asked>> {
        self.reads
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl ObjectStore for CountingStore {
    async fn put(&self, name: &str, body: ByteStream) -> StoreResult<UploadedObject> {
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
        self.locked().push(match &range {
            Some(range) => Asked::Range(range.clone()),
            None => Asked::Whole,
        });
        self.inner.get(object, range).await
    }

    async fn list(&self, page: Option<&PageToken>) -> StoreResult<ObjectPage> {
        self.locked().push(Asked::Listed);
        self.inner.list(page).await
    }

    async fn trash(&self, object: &ObjectRef) -> StoreResult<()> {
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> StoreResult<()> {
        self.inner.purge(object).await
    }
}
