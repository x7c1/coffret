use std::ops::Range;
use std::sync::Mutex;

use async_trait::async_trait;
use coffret_model::ObjectRef;

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;

/// A store that remembers what a commit wrote, wrapped around the real one.
///
/// The repair cases are about which objects reached Storage rather than about
/// what the flow answered: a commit that finds the committed Keyring set
/// complete must rewrite none of it, one that finds it short must rewrite the
/// positions it lost and no others, and one that may not repair at all — a
/// replica Storage would not hand over, a generation no replica answers for —
/// must write over nothing (spec: KL-13, KL-16). What the flow returns cannot
/// settle any of that — a repair reporting that it rewrote nothing would be
/// believed by a case that only read the outcome — so the cases read the names
/// off the store instead.
///
/// It wraps whatever store the backend handed the suite, so the cases count real
/// requests against a real provider exactly as they count them in memory.
pub(super) struct CountingStore<'a> {
    inner: &'a dyn ObjectStore,
    written: Mutex<Vec<String>>,
}

impl<'a> CountingStore<'a> {
    /// Starts remembering at nothing.
    pub(super) fn around(inner: &'a dyn ObjectStore) -> Self {
        Self {
            inner,
            written: Mutex::new(Vec::new()),
        }
    }

    /// Every name the unconditional write was aimed at, in the order it was.
    ///
    /// Only that write, because it is the only one a Keyring replica is ever
    /// made by: a record and a Snapshot go through the conditional create, and a
    /// case about what a repair wrote would count them as noise (spec: CP-3,
    /// KL-14).
    pub(super) fn written(&self) -> Vec<String> {
        self.written
            .lock()
            .expect("the counting store's own lock is never poisoned")
            .clone()
    }
}

#[async_trait]
impl ObjectStore for CountingStore<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<ObjectRef> {
        self.written
            .lock()
            .expect("the counting store's own lock is never poisoned")
            .push(name.to_owned());
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
