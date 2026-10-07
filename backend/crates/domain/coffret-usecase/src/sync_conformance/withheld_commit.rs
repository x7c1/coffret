use std::ops::Range;

use async_trait::async_trait;
use coffret_model::{ControlObjectName, ObjectRef};

use crate::byte_stream::ByteStream;

use crate::commit_slot::CommitSlot;
use crate::error::{Error, Result};
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::uploaded_object::UploadedObject;

/// Accepts the Journal create but loses its response and withholds that head
/// from later listings. A caller sees the older history (spec: OC-1, OC-3).
pub(super) struct WithheldCommit<'a> {
    pub(super) inner: &'a dyn ObjectStore,
}

fn is_head(name: &str) -> bool {
    matches!(
        ControlObjectName::parse(name),
        Ok(ControlObjectName::Head { .. })
    )
}

#[async_trait]
impl ObjectStore for WithheldCommit<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<UploadedObject> {
        self.inner.put(name, body).await
    }
    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        self.inner.reserve_create(name).await
    }
    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        if is_head(slot.name()) {
            self.inner.put_if_absent(slot, body).await?;
            return Err(Error::Transport {
                detail: "the commit response was lost".to_owned(),
                source: None,
            });
        }
        self.inner.put_if_absent(slot, body).await
    }
    fn object_at(&self, slot: &CommitSlot) -> Result<ObjectRef> {
        self.inner.object_at(slot)
    }

    async fn get(&self, object: &ObjectRef, range: Option<Range<u64>>) -> Result<ByteStream> {
        self.inner.get(object, range).await
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        let mut result = self.inner.list(page).await?;
        result.objects.retain(|object| !is_head(&object.name));
        Ok(result)
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        self.inner.trash(object).await
    }
    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}
