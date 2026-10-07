use std::ops::Range;
use std::time::Duration;

use async_trait::async_trait;
use coffret_model::ObjectRef;
use tokio::io::AsyncReadExt;

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::uploaded_object::UploadedObject;

/// How many pieces a slow put pulls its body in.
pub(super) const PIECES: u64 = 4;

/// A store whose writes pull the body a piece at a time, waiting between
/// pieces, around the real one.
///
/// What a Pack of tens of megabytes on its way to Drive looks like from the use
/// case: a put that takes the stream as the network takes it, for long enough
/// that a run reporting only when a put answers would say nothing in between.
/// The bytes are handed on to the real store whole once they have all been
/// pulled, so the object that lands is the one that was sent.
///
/// Only [`put`](ObjectStore::put) is slow: the control objects a commit writes
/// go through [`put_if_absent`](ObjectStore::put_if_absent), and a run's own
/// reports are about the Packs.
pub(super) struct SlowStore<'a> {
    inner: &'a dyn ObjectStore,
    pause: Duration,
}

impl<'a> SlowStore<'a> {
    /// Pauses `pause` before each piece of every put.
    pub(super) fn around(inner: &'a dyn ObjectStore, pause: Duration) -> Self {
        Self { inner, pause }
    }
}

#[async_trait]
impl ObjectStore for SlowStore<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<UploadedObject> {
        let len = body.len();
        let piece = len.div_ceil(PIECES).max(1);
        let mut reader = body.into_reader();
        let mut bytes = Vec::new();
        loop {
            tokio::time::sleep(self.pause).await;
            let read = (&mut reader).take(piece).read_to_end(&mut bytes).await?;
            if read == 0 {
                break;
            }
        }
        self.inner.put(name, ByteStream::from(bytes)).await
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
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}
