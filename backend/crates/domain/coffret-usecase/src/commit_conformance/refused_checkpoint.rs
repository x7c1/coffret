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

/// Refuses checkpoint creation while allowing Journal commits (spec: CK-8).
pub(super) struct RefusedCheckpoint<'a> {
    pub(super) inner: &'a dyn ObjectStore,
}

fn is_snapshot(name: &str) -> bool {
    matches!(
        ControlObjectName::parse(name),
        Ok(ControlObjectName::IndexSnapshot { .. })
    )
}

#[async_trait]
impl ObjectStore for RefusedCheckpoint<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<UploadedObject> {
        self.inner.put(name, body).await
    }
    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        self.inner.reserve_create(name).await
    }
    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        if is_snapshot(slot.name()) {
            return Err(Error::PermissionDenied {
                detail: "checkpoint writes refused".to_owned(),
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
        self.inner.list(page).await
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        self.inner.trash(object).await
    }
    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}
