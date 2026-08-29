use std::ops::Range;

use async_trait::async_trait;
use coffret_model::ObjectRef;

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;

/// A store that hands back damaged bytes for one object.
///
/// Bytes that go astray between Storage and a device cannot be arranged by
/// driving the flow, and writing damage into the bucket beforehand would test
/// something else — the fetch would then be refusing an object the *Library*
/// holds wrongly, rather than one that arrived wrongly. What is being checked is
/// that a device compares what arrived against the hash its Journal record
/// carries and refuses the difference (spec: FM-15, CP-11), which only shows if
/// the difference appears in transit.
///
/// One object rather than all of them, because the fetch reads control objects
/// through the same port: damaging those would stop the run before it ever
/// reached a Container.
///
/// It wraps whatever store the backend handed the suite, so the case runs against
/// a real provider exactly as it runs in memory.
pub(super) struct ManglingStore<'a> {
    inner: &'a dyn ObjectStore,
    mangled: ObjectRef,
    /// Where in the object the damage starts being done.
    from: u64,
}

impl<'a> ManglingStore<'a> {
    /// Damages every read of `mangled` and passes everything else through.
    pub(super) fn around(inner: &'a dyn ObjectStore, mangled: ObjectRef) -> Self {
        Self {
            inner,
            mangled,
            from: 0,
        }
    }

    /// The same, damaging only reads that start at or beyond `from`.
    ///
    /// A partial fetch reads an object in three pieces — its header, its meta
    /// section, and the chunks covering one Entry — and the case about a damaged
    /// chunk is about the third. Damaging the first two as well would have the
    /// run refuse the object before it had aimed a read at a chunk at all, which
    /// is a different refusal (spec: FM-2, FM-8).
    pub(super) fn beyond(inner: &'a dyn ObjectStore, mangled: ObjectRef, from: u64) -> Self {
        Self {
            inner,
            mangled,
            from,
        }
    }

    /// Whether one read of one object is a read this store damages.
    fn damages(&self, object: &ObjectRef, range: Option<&Range<u64>>) -> bool {
        object == &self.mangled && range.map_or(0, |range| range.start) >= self.from
    }
}

#[async_trait]
impl ObjectStore for ManglingStore<'_> {
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
        let damages = self.damages(object, range.as_ref());
        let stream = self.inner.get(object, range).await?;
        if !damages {
            return Ok(stream);
        }
        // One byte, and the length left alone: a short answer would be caught as
        // a length mismatch by the port itself, which is a different verdict from
        // the one this case is about.
        let mut bytes = stream.into_bytes().await?;
        if let Some(byte) = bytes.last_mut() {
            *byte ^= 0xff;
        }
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
