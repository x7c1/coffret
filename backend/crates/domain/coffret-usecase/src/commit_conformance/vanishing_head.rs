use std::ops::Range;
use std::sync::Mutex;

use async_trait::async_trait;
use coffret_model::ObjectRef;

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::error_chains::every_link;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::uploaded_object::UploadedObject;

/// A store that permanently deletes one head the first time a commit reserves
/// a slot, wrapped around the real one.
///
/// Reserving the slots the new record will carry is the last thing a commit
/// does before it re-reads the head its own slot came from (spec: CP-16), and
/// it comes after the catch-up has listed and read that head. So the head is
/// gone at exactly the moment the re-read exists for: after everything the
/// writer learned about the Library, before the slot is consumed. A later
/// epoch's rotation is what deletes old heads in a real Library (spec: MR-3);
/// the case deletes it directly, through the same permanent removal.
///
/// It wraps whatever store the backend handed the suite, so the head vanishes
/// from a real provider exactly as it does in memory.
pub(super) struct VanishingHead<'a> {
    inner: &'a dyn ObjectStore,
    head: Mutex<Option<ObjectRef>>,
}

impl<'a> VanishingHead<'a> {
    /// Purges `head` from `inner` at the first slot reservation, and only then.
    pub(super) fn purging(inner: &'a dyn ObjectStore, head: ObjectRef) -> Self {
        Self {
            inner,
            head: Mutex::new(Some(head)),
        }
    }

    /// Whether the head has been purged yet.
    pub(super) fn has_purged(&self) -> bool {
        self.head
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_none()
    }

    fn take_head(&self) -> Option<ObjectRef> {
        self.head
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

#[async_trait]
impl ObjectStore for VanishingHead<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<UploadedObject> {
        self.inner.put(name, body).await
    }

    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        // The lock is released before the purge goes out, so a reservation
        // made while it is in flight finds the head already taken.
        if let Some(head) = self.take_head() {
            self.inner.purge(&head).await.unwrap_or_else(|error| {
                panic!("purging the head must succeed: {}", every_link(&error))
            });
        }
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
