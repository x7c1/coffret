use std::ops::Range;
use std::sync::Mutex;

use async_trait::async_trait;
use coffret_model::ObjectRef;

use crate::byte_stream::ByteStream;
use crate::commit_conformance::is_head;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::progress::Step;
use crate::recorded_progress::Recording;
use crate::uploaded_object::UploadedObject;

/// What a run had last said when it sent one object to Storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Seen {
    /// A Keyring replica was sent, with the run's last step at that moment.
    Replica(Option<Step>),
    /// The head was sent, with the run's last step at that moment.
    Head(Option<Step>),
}

/// A store that notes what the run had last reported each time a commit sends
/// it an object, wrapped around the real one.
pub(super) struct WatchingStore<'a> {
    inner: &'a dyn ObjectStore,
    progress: &'a Recording,
    seen: Mutex<Vec<Seen>>,
}

impl<'a> WatchingStore<'a> {
    /// Notes what `progress` had last been told at each write to `inner`.
    pub(super) fn around(inner: &'a dyn ObjectStore, progress: &'a Recording) -> Self {
        Self {
            inner,
            progress,
            seen: Mutex::new(Vec::new()),
        }
    }

    /// Every write a commit sent, in order, with what had been said by then.
    pub(super) fn seen(&self) -> Vec<Seen> {
        self.seen
            .lock()
            .expect("the watching store's own lock is never poisoned")
            .clone()
    }

    fn note(&self, seen: Seen) {
        self.seen
            .lock()
            .expect("the watching store's own lock is never poisoned")
            .push(seen);
    }
}

#[async_trait]
impl ObjectStore for WatchingStore<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<UploadedObject> {
        // The unconditional write is the one a Keyring replica is made by; a
        // record and a Snapshot go through the conditional create.
        self.note(Seen::Replica(self.progress.steps().last().copied()));
        self.inner.put(name, body).await
    }

    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        self.inner.reserve_create(name).await
    }

    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        if is_head(slot.name()) {
            self.note(Seen::Head(self.progress.steps().last().copied()));
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
