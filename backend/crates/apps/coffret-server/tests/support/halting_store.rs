use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use coffret_model::ObjectRef;
use coffret_usecase::{
    ByteStream, CommitSlot, Error, ObjectPage, ObjectStore, PageToken, Result as StoreResult,
};

/// A store that can be made to stop answering, and to answer again.
///
/// What one case is about is a fill meeting a Storage that has gone — a bucket
/// that is unreachable, a grant that has run out — which is the one failure a
/// fill stops for rather than records: every further Entry would meet it
/// identically. The refusal is [`Error::Unauthenticated`] because it is the
/// clearest of them and, like the rest of that half of the vocabulary, is not
/// retryable — so a case drives it without a retry loop waiting anything out.
///
/// Only reads are refused. A fill's first Storage call for an Entry is a read,
/// so refusing them is enough to stop it where it would be stopped, and leaving
/// the rest alone keeps this a case's switch rather than a second store.
pub struct HaltingStore {
    inner: std::sync::Arc<dyn ObjectStore>,
    halted: AtomicBool,
    refused: AtomicUsize,
}

impl HaltingStore {
    /// Answering, until told otherwise.
    pub fn around(inner: std::sync::Arc<dyn ObjectStore>) -> Self {
        Self {
            inner,
            halted: AtomicBool::new(false),
            refused: AtomicUsize::new(0),
        }
    }

    /// Refuses every read from now on.
    pub fn halt(&self) {
        self.halted.store(true, Ordering::SeqCst);
    }

    /// Answers again.
    pub fn resume(&self) {
        self.halted.store(false, Ordering::SeqCst);
    }

    /// How many reads have been refused.
    ///
    /// What a case counts to know that a fill stopped rather than pressed on: a
    /// fill of a folder of two files that tried both would be refused twice.
    pub fn refused(&self) -> usize {
        self.refused.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ObjectStore for HaltingStore {
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
        if self.halted.load(Ordering::SeqCst) {
            self.refused.fetch_add(1, Ordering::SeqCst);
            return Err(Error::Unauthenticated {
                detail: "the grant has run out".to_owned(),
            });
        }
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
