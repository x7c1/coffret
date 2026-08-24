use std::ops::Range;

use async_trait::async_trait;
use coffret_model::ObjectRef;

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::object_info::ObjectInfo;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::provider_hash::ProviderHash;

/// The digest the wrapper answers with instead of the real one.
///
/// A well-formed MD5 that is not the digest of anything a case uploads, so what
/// the run meets is a disagreement and not a malformed answer.
const WRONG: &str = "00000000000000000000000000000000";

/// A store that reports the wrong digest for what it stored.
///
/// A provider that lost bytes in transit and reports the digest of what
/// actually landed cannot be reached by driving the flow, and setting the state
/// up afterwards would test something else: what is being checked is that the
/// run *stops* before the batch names the object, which only shows if the
/// disagreement is there while it is running.
///
/// It wraps whatever store the backend handed the suite, so the case runs
/// against a real provider exactly as it runs in memory.
pub(super) struct ManglingStore<'a> {
    inner: &'a dyn ObjectStore,
}

impl<'a> ManglingStore<'a> {
    /// Answers every listing with a digest that is nobody's bytes.
    pub(super) fn around(inner: &'a dyn ObjectStore) -> Self {
        Self { inner }
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
        self.inner.get(object, range).await
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        let page = self.inner.list(page).await?;
        Ok(ObjectPage {
            objects: page
                .objects
                .into_iter()
                .map(|object| ObjectInfo {
                    hash: Some(ProviderHash::new(WRONG)),
                    ..object
                })
                .collect(),
            next: page.next,
        })
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}
