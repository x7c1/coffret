use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};

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
/// One case is about a cost rather than an outcome: a run reads the Library's
/// head before it reads the catalog, and the ordinary run — the one with nothing
/// to settle and nothing to commit — may not be made to pay for reading it more
/// than that once. Nothing the flow returns says how often it read the head, so
/// the case counts the walks of the listing instead, which is what reading the
/// Library's control state takes (spec: FM-12, CK-9).
///
/// A walk and not a page: how many pages one walk takes is the backend's answer
/// and not the flow's, so a page continuing a walk that is already under way is
/// not a second reading of the head.
///
/// It wraps whatever store the backend handed the suite, so the case counts real
/// requests against a real provider exactly as it counts them in memory.
pub(super) struct CountingStore<'a> {
    inner: &'a dyn ObjectStore,
    walks: AtomicUsize,
}

impl<'a> CountingStore<'a> {
    /// Starts counting at nothing.
    pub(super) fn around(inner: &'a dyn ObjectStore) -> Self {
        Self {
            inner,
            walks: AtomicUsize::new(0),
        }
    }

    /// How many walks of the listing have been started since.
    pub(super) fn walks(&self) -> usize {
        self.walks.load(Ordering::Relaxed)
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
        self.inner.get(object, range).await
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        if page.is_none() {
            self.walks.fetch_add(1, Ordering::Relaxed);
        }
        self.inner.list(page).await
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}
