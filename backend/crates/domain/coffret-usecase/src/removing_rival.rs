//! Another device removing Containers while a run is still preparing its batch.
//!
//! The sync, freeze, and delete suites each have one case about a removal that
//! stopped being current between prepare and commit (spec: CP-18), and the
//! rival is the same in all three: it commits a batch that removes the
//! Containers and adds nothing, through the same [`commit_batch`] every writer
//! uses.

use std::ops::Range;
use std::sync::Mutex;

use async_trait::async_trait;
use coffret_model::{ContainerId, ControlObjectName, ObjectRef};

use crate::byte_stream::ByteStream;
use crate::commit::{commit_batch, CommitPolicy, CommitRequest, ControlKeys, PreparedBatch};
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::in_memory_index::InMemoryIndex;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::uploaded_object::UploadedObject;

/// A store that lets another device remove Containers just before the run under
/// test uploads its first one.
///
/// That moment is after the run read the head and planned its batch, and before
/// its commit catches up again: exactly the window in which a removal the batch
/// names can stop being current. The rival catches up on a catalog of its own,
/// the way a second device would, and commits through the inner store, so the
/// head the run meets is one a real commit produced.
pub(crate) struct RemovingRival<'a> {
    inner: &'a dyn ObjectStore,
    keys: &'a ControlKeys,
    policy: CommitPolicy,
    removals: Mutex<Option<Vec<ContainerId>>>,
}

impl<'a> RemovingRival<'a> {
    /// Wraps `inner` so that a rival removes `removals` before the first
    /// Container upload, and only then.
    pub(crate) fn removing(
        inner: &'a dyn ObjectStore,
        keys: &'a ControlKeys,
        policy: CommitPolicy,
        removals: Vec<ContainerId>,
    ) -> Self {
        Self {
            inner,
            keys,
            policy,
            removals: Mutex::new(Some(removals)),
        }
    }

    /// Whether the rival has had its turn.
    pub(crate) fn has_removed(&self) -> bool {
        self.removals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_none()
    }

    fn take_removals(&self) -> Option<Vec<ContainerId>> {
        self.removals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

#[async_trait]
impl ObjectStore for RemovingRival<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<UploadedObject> {
        // A Container's object name is not a control object's, and a Container
        // is the first thing a run puts on Storage of its own (spec: FM-3,
        // FM-12). The lock is released before the rival commits through the
        // inner store, which never comes back here.
        if ControlObjectName::parse(name).is_err() {
            if let Some(removals) = self.take_removals() {
                let rival = InMemoryIndex::new();
                commit_batch(
                    CommitRequest::new(
                        self.inner,
                        &rival,
                        self.keys,
                        PreparedBatch::default().removing(removals),
                    )
                    .with_policy(self.policy.clone()),
                )
                .await
                .expect("the rival removes current Containers uncontested and must commit");
            }
        }
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
        self.inner.list(page).await
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}
