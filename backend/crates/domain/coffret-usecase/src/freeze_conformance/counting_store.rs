use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Mutex;

use async_trait::async_trait;
use coffret_model::{ContainerId, ObjectRef};

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;

/// A store that records which objects a run touched, wrapped around the real
/// one.
///
/// One case is about a cost and an absence rather than an outcome: a second
/// freeze over the same folder must leave every existing Pack byte-for-byte
/// unchanged, and must not have read one either — a freeze neither takes an
/// existing Pack as input nor rewrites it (spec: PK-1, PK-2). Nothing the flow
/// returns proves that, so the case names the objects instead.
///
/// It records names rather than counts because the claim is about *which*
/// objects: a run still reads its own control state and still writes the
/// Journal record and the Keyring replicas, so a bare tally would say nothing.
///
/// It wraps whatever store the backend handed the suite, so the case observes
/// real requests against a real provider exactly as it observes them in memory.
pub(super) struct CountingStore<'a> {
    inner: &'a dyn ObjectStore,
    written: Mutex<BTreeSet<String>>,
    read: Mutex<BTreeSet<ObjectRef>>,
}

impl<'a> CountingStore<'a> {
    /// Starts with nothing recorded.
    pub(super) fn around(inner: &'a dyn ObjectStore) -> Self {
        Self {
            inner,
            written: Mutex::new(BTreeSet::new()),
            read: Mutex::new(BTreeSet::new()),
        }
    }

    /// Whether anything was written under one Container's object name.
    pub(super) fn wrote(&self, container_id: ContainerId) -> bool {
        self.written
            .lock()
            .expect("the recording lock is never poisoned")
            .contains(&container_id.object_name())
    }

    /// Whether one Container's object was fetched.
    ///
    /// The handle is the case's to supply, because a store that mints
    /// identifiers does not name objects by their names (spec: FM-3).
    pub(super) fn read_object(&self, object: &ObjectRef) -> bool {
        self.read
            .lock()
            .expect("the recording lock is never poisoned")
            .contains(object)
    }

    /// How many objects were written in all.
    pub(super) fn writes(&self) -> usize {
        self.written
            .lock()
            .expect("the recording lock is never poisoned")
            .len()
    }

    fn record_write(&self, name: &str) {
        self.written
            .lock()
            .expect("the recording lock is never poisoned")
            .insert(name.to_owned());
    }
}

#[async_trait]
impl ObjectStore for CountingStore<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<ObjectRef> {
        self.record_write(name);
        self.inner.put(name, body).await
    }

    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        self.inner.reserve_create(name).await
    }

    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        self.record_write(slot.name());
        self.inner.put_if_absent(slot, body).await
    }

    fn object_at(&self, slot: &CommitSlot) -> Result<ObjectRef> {
        self.inner.object_at(slot)
    }

    async fn get(&self, object: &ObjectRef, range: Option<Range<u64>>) -> Result<ByteStream> {
        self.read
            .lock()
            .expect("the recording lock is never poisoned")
            .insert(object.clone());
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
