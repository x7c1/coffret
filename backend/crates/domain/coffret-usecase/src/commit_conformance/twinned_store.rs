use std::ops::Range;

use async_trait::async_trait;
use coffret_model::{ControlObjectName, ObjectRef};

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::object_info::ObjectInfo;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::uploaded_object::UploadedObject;

/// A store that lists one name twice, wrapped around the real one.
///
/// On a Storage whose names are not unique one Keyring replica name can carry
/// two objects — a repair writes a valid copy beside one that did not read back
/// valid — and which of the two a provider lists last is the provider's
/// business (spec: KL-14). A store keyed by name cannot hold two objects under
/// one name at all, and a store that mints identifiers lists them in an order of
/// its own, so neither can put the damaged copy on the side of the valid one a
/// case needs. This puts it there: a second object under `name`, whose bytes the
/// wrapper serves itself, listed right before or right after the real one.
///
/// It wraps whatever store the backend handed the suite, so the case runs
/// against a real provider exactly as it runs in memory, and the twin never
/// reaches the provider: nothing is left for the case to clean up.
pub(super) struct TwinnedStore<'a> {
    inner: &'a dyn ObjectStore,
    name: String,
    twin: ObjectRef,
    bytes: Vec<u8>,
    placement: Placement,
}

/// Where the twin is listed, next to the object the store under test holds.
#[derive(Debug, Clone, Copy)]
pub(super) enum Placement {
    /// Right before it, so the real object is the one listed last.
    First,
    /// Right after it, so the twin is the one listed last.
    Last,
}

impl<'a> TwinnedStore<'a> {
    /// Lists an object holding `bytes` under `name`, beside the one already
    /// there.
    pub(super) fn beside(
        inner: &'a dyn ObjectStore,
        name: &ControlObjectName,
        bytes: Vec<u8>,
        placement: Placement,
    ) -> Self {
        let name = name.to_string();
        Self {
            inner,
            twin: ObjectRef::new(format!("twin-of-{name}")),
            name,
            bytes,
            placement,
        }
    }

    fn twin_info(&self) -> ObjectInfo {
        ObjectInfo {
            object_ref: self.twin.clone(),
            name: self.name.clone(),
            hash: None,
        }
    }
}

#[async_trait]
impl ObjectStore for TwinnedStore<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<UploadedObject> {
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
        if object != &self.twin {
            return self.inner.get(object, range).await;
        }
        let len = self.bytes.len();
        let range = range.map_or(0..len, |range| {
            let clamp = |at: u64| usize::try_from(at).map_or(len, |at| at.min(len));
            clamp(range.start)..clamp(range.end)
        });
        Ok(ByteStream::from(
            self.bytes.get(range).unwrap_or_default().to_vec(),
        ))
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        let listed = self.inner.list(page).await?;
        let mut objects = Vec::with_capacity(listed.objects.len() + 1);
        for object in listed.objects {
            let twinned = object.name == self.name;
            if twinned && matches!(self.placement, Placement::First) {
                objects.push(self.twin_info());
            }
            objects.push(object);
            if twinned && matches!(self.placement, Placement::Last) {
                objects.push(self.twin_info());
            }
        }
        Ok(ObjectPage {
            objects,
            next: listed.next,
        })
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        // The twin is never on the provider, so there is nothing to trash.
        if object == &self.twin {
            return Ok(());
        }
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        if object == &self.twin {
            return Ok(());
        }
        self.inner.purge(object).await
    }
}
