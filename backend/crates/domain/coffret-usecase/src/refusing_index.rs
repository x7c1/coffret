use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use coffret_model::{
    ContainerId, ContainerSummary, EntryLocation, EntryPath, IndexCheckpoint, JournalRecord,
    SnapshotContent,
};

use crate::committed_batch::CommittedBatch;
use crate::device_state::{DeviceTime, LocalEntry, LocalObservation, Mapping, PendingUpload};
use crate::in_memory_index::InMemoryIndex;
use crate::index::Index;
use crate::index_error::{IndexError, IndexResult};

/// A catalog that cannot say what this device maps, and answers everything
/// else honestly.
///
/// The one question every door onto the EP-9 translation asks first, and the
/// one a fetch asks straight after its catch-up — which is what makes it the
/// question to refuse: the catch-up itself goes through, so what a caller meets
/// is the catalog failing inside the fetch's own vocabulary rather than inside
/// the commit's (spec: EP-9, CK-9). A backend fault rather than anything about
/// a mapping, because that is the shape of a catalog that could not be used.
///
/// Refusing from the start, or from the moment a case says so. The second is
/// what a case over a Library that already holds Entries needs: the fixture
/// records its mappings and replays the Library through this very catalog, and
/// only then is the catalog taken away — so what the case meets is a device
/// whose catalog went bad under it, rather than one that never had one.
pub struct RefusingIndex {
    inner: InMemoryIndex,
    refusing: AtomicBool,
}

impl RefusingIndex {
    /// An empty catalog that refuses to list its mappings.
    pub fn new() -> Self {
        Self {
            inner: InMemoryIndex::new(),
            refusing: AtomicBool::new(true),
        }
    }

    /// `inner`, answering honestly until [`refuse`](Self::refuse) is called.
    pub fn around(inner: InMemoryIndex) -> Self {
        Self {
            inner,
            refusing: AtomicBool::new(false),
        }
    }

    /// Refuses to list the mappings from now on.
    pub fn refuse(&self) {
        self.refusing.store(true, Ordering::SeqCst);
    }
}

impl Default for RefusingIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Index for RefusingIndex {
    async fn restore(&self, snapshot: SnapshotContent) -> IndexResult<()> {
        self.inner.restore(snapshot).await
    }

    async fn apply(&self, record: JournalRecord) -> IndexResult<()> {
        self.inner.apply(record).await
    }

    async fn refresh(&self, batch: CommittedBatch) -> IndexResult<()> {
        self.inner.refresh(batch).await
    }

    async fn snapshot(&self) -> IndexResult<SnapshotContent> {
        self.inner.snapshot().await
    }

    async fn checkpoint(&self) -> IndexResult<Option<IndexCheckpoint>> {
        self.inner.checkpoint().await
    }

    async fn entry_at(&self, path: &EntryPath) -> IndexResult<Option<EntryLocation>> {
        self.inner.entry_at(path).await
    }

    async fn entries_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<EntryLocation>> {
        self.inner.entries_under(prefix).await
    }

    async fn containers_under(
        &self,
        prefix: Option<&EntryPath>,
    ) -> IndexResult<Vec<ContainerSummary>> {
        self.inner.containers_under(prefix).await
    }

    async fn set_mapping(&self, mapping: Mapping) -> IndexResult<()> {
        self.inner.set_mapping(mapping).await
    }

    async fn mappings(&self) -> IndexResult<Vec<Mapping>> {
        if self.refusing.load(Ordering::SeqCst) {
            return Err(IndexError::Backend {
                operation: "reading the mappings",
                cause: Box::new(std::io::Error::other(
                    "the catalog's file went away under the process",
                )),
            });
        }
        self.inner.mappings().await
    }

    async fn mark_present(&self, observation: LocalObservation) -> IndexResult<()> {
        self.inner.mark_present(observation).await
    }

    async fn mark_absent(&self, path: &EntryPath, at: DeviceTime) -> IndexResult<()> {
        self.inner.mark_absent(path, at).await
    }

    async fn local_entry_at(&self, path: &EntryPath) -> IndexResult<Option<LocalEntry>> {
        self.inner.local_entry_at(path).await
    }

    async fn present_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<LocalEntry>> {
        self.inner.present_under(prefix).await
    }

    async fn present_without_entry(&self) -> IndexResult<Vec<LocalEntry>> {
        self.inner.present_without_entry().await
    }

    async fn record_pending_upload(&self, pending: PendingUpload) -> IndexResult<()> {
        self.inner.record_pending_upload(pending).await
    }

    async fn mark_spooled(&self, container_id: ContainerId) -> IndexResult<()> {
        self.inner.mark_spooled(container_id).await
    }

    async fn clear_pending_upload(&self, container_id: ContainerId) -> IndexResult<()> {
        self.inner.clear_pending_upload(container_id).await
    }

    async fn pending_uploads(&self) -> IndexResult<Vec<PendingUpload>> {
        self.inner.pending_uploads().await
    }
}
