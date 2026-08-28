use async_trait::async_trait;
use coffret_model::{
    ContainerId, ContainerSummary, EntryLocation, EntryPath, IndexCheckpoint, JournalRecord,
    SnapshotContent,
};

use crate::committed_batch::CommittedBatch;
use crate::device_state::{DeviceTime, LocalEntry, LocalObservation, Mapping, PendingUpload};
use crate::index::Index;
use crate::index_error::{IndexError, IndexResult};

/// A catalog that refuses the one write only the committer can do, wrapped
/// around the real one.
///
/// [`Index::refresh`] is the post-commit step whose failure leaves the device
/// out of step with a Library that has already changed, and that state cannot be
/// reached by driving the flow: the record has to land and the refresh has to
/// fail, in that order, inside one call. Setting the state up by hand afterwards
/// would test something else — what is being checked is that the run stops there
/// and that what it leaves behind is enough for the next one to finish the job.
///
/// Everything else is passed straight through to the catalog the backend handed
/// the suite, which is the same catalog the run afterwards uses directly: what
/// the interrupted run wrote down really is what the next one finds.
pub(super) struct RefusingIndex<'a> {
    inner: &'a dyn Index,
}

impl<'a> RefusingIndex<'a> {
    /// Refuses every [`refresh`](Index::refresh) and answers everything else
    /// honestly.
    pub(super) fn around(inner: &'a dyn Index) -> Self {
        Self { inner }
    }
}

/// What the refused refresh reports.
///
/// A backend fault rather than anything about the batch: the catalog's own store
/// is what failed, which is the one shape of this failure that says nothing is
/// wrong with what was committed.
fn refused() -> IndexError {
    IndexError::Backend {
        operation: "refresh",
        cause: Box::new(std::io::Error::other(
            "the catalog was interrupted after the batch's record landed",
        )),
    }
}

#[async_trait]
impl Index for RefusingIndex<'_> {
    async fn restore(&self, snapshot: SnapshotContent) -> IndexResult<()> {
        self.inner.restore(snapshot).await
    }

    async fn apply(&self, record: JournalRecord) -> IndexResult<()> {
        self.inner.apply(record).await
    }

    async fn refresh(&self, _batch: CommittedBatch) -> IndexResult<()> {
        Err(refused())
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
