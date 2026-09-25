use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use coffret_model::ObjectRef;

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;

/// A store that answers one ranged read of one object a byte short, once.
///
/// What a proxy that cut a transfer short leaves, or a provider answering a
/// range with less than was asked for: a stream that keeps to the length it
/// declares, and declares less than the range. Nothing in the bucket is wrong,
/// so the case it serves is about which channel that answer travels in — it is
/// Storage's doing and worth asking again, where a Container whose own header
/// lies about its lengths is not.
///
/// Once rather than every time, so that the attempt after it gets the whole
/// answer and the case can see the fetch come through. It wraps whatever store
/// the backend handed the suite, so the short answer is made in transit against
/// a real provider exactly as in memory.
pub(super) struct ShorteningStore<'a> {
    inner: &'a dyn ObjectStore,
    shortened: ObjectRef,
    /// Where in the object a read has to start for this store to shorten it.
    from: u64,
    /// Whether the one short answer has been given.
    spent: AtomicBool,
}

impl<'a> ShorteningStore<'a> {
    /// Shortens the first read of `shortened` that starts at or beyond `from`,
    /// and passes everything else through.
    ///
    /// A partial fetch reads an object's header and meta section before the
    /// chunks covering one Entry, and the case is about the third read: a front
    /// that came back short is refused before a chunk is aimed at.
    pub(super) fn beyond(inner: &'a dyn ObjectStore, shortened: ObjectRef, from: u64) -> Self {
        Self {
            inner,
            shortened,
            from,
            spent: AtomicBool::new(false),
        }
    }

    /// Whether this store has given its one short answer.
    pub(super) fn has_shortened(&self) -> bool {
        self.spent.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl ObjectStore for ShorteningStore<'_> {
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
        let aimed = object == &self.shortened
            && range.as_ref().is_some_and(|range| range.start >= self.from);
        let stream = self.inner.get(object, range).await?;
        if !aimed || self.spent.swap(true, Ordering::Relaxed) {
            return Ok(stream);
        }
        // A byte short, and the declared length with it: an answer that
        // disagreed with its own declaration would be caught as that, which
        // is a different refusal from the one this case is about.
        let mut bytes = stream.into_bytes().await?;
        bytes.pop();
        Ok(ByteStream::from(bytes))
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
