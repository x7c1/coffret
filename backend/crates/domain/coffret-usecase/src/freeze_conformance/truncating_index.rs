use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use coffret_model::{
    ContainerId, ContainerSummary, EntryLocation, EntryPath, IndexCheckpoint, JournalRecord,
    SnapshotContent,
};

use crate::committed_batch::CommittedBatch;
use crate::device_state::{
    DeviceTime, LocalEntry, LocalObservation, Mapping, PendingUpload, SpoolState,
};
use crate::index::Index;
use crate::index_error::IndexResult;

/// A catalog that shortens one local file the moment the Pack holding it is
/// announced.
///
/// The window the freeze's own guard is about is inside one call, which is why
/// nothing planted beforehand reaches it. A Pack's entry table is settled by the
/// scan and written before the content streams (spec: PK-3), so a file that
/// stops being the file the scan measured has to stop being it *after* the scan
/// and *before* the read — and the pending row, which is recorded before the
/// first byte of the spool (spec: OC-2), is the one moment inside that window a
/// case can reach at all.
///
/// Truncation rather than a rewrite, because it is the shape of the accident the
/// guard exists for: a file still being written while the run reads it. It is
/// also the one that reaches the run's own length measurement, the detection
/// site with no format error in hand.
///
/// Everything else is passed straight through to the catalog the backend handed
/// the suite, so what the stopped run left behind really is what it wrote down.
pub(crate) struct TruncatingIndex<'a> {
    inner: &'a dyn Index,
    path: PathBuf,
    keep: u64,
    shortened: AtomicBool,
}

impl<'a> TruncatingIndex<'a> {
    /// Cuts one file down to `keep` bytes as the first Pack is announced.
    pub(crate) fn shortening(inner: &'a dyn Index, path: &Path, keep: u64) -> Self {
        Self {
            inner,
            path: path.to_path_buf(),
            keep,
            shortened: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl Index for TruncatingIndex<'_> {
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

    /// Shortens the file as the first Pack is announced, then records the row.
    ///
    /// A panic rather than a returned error: a case that could not arrange the
    /// change has not tested the guard, and saying so as an `IndexError` would
    /// let the run report something else entirely.
    async fn record_pending_upload(&self, pending: PendingUpload) -> IndexResult<()> {
        if pending.state == SpoolState::Spooling && !self.shortened.swap(true, Ordering::Relaxed) {
            let file = std::fs::File::options()
                .write(true)
                .open(&self.path)
                .expect("opening the file to shorten it must succeed");
            file.set_len(self.keep)
                .expect("shortening the file must succeed");
        }
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
