use std::sync::atomic::{AtomicUsize, Ordering};

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
use crate::index_error::{IndexError, IndexResult};

/// A catalog that holds every spool step to the ordering it promises, and can
/// stop one where the ordering matters most.
///
/// Two things no flow produces and no state planted afterwards could show. One is
/// the ordering itself: that the row naming a spool was written *before* the file
/// existed is a claim about a moment inside one call, so it is checked from
/// inside that call — every Spooling row is asserted against the disk as it
/// arrives, and those rows are counted so a case can say one was announced per
/// Container the run spooled. The other is the interruption that leaves a spool
/// file with a Spooling row over it, which is the one failure point where a
/// run leaves ciphertext on disk that no batch will ever name.
///
/// Everything else is passed straight through to the catalog the backend handed
/// the suite, which is the same catalog the run afterwards uses directly: what
/// the interrupted run wrote down really is what the next one finds.
///
/// The freeze suite borrows it rather than writing a second copy — the Pack spool
/// step has the same ordering to keep, and one account of what it means is
/// enough.
pub(crate) struct WatchingIndex<'a> {
    inner: &'a dyn Index,
    spooling: AtomicUsize,
    refuse_mark_spooled: bool,
}

impl<'a> WatchingIndex<'a> {
    /// Watches every spool announcement and lets the run finish.
    pub(crate) fn around(inner: &'a dyn Index) -> Self {
        Self {
            inner,
            spooling: AtomicUsize::new(0),
            refuse_mark_spooled: false,
        }
    }

    /// The same, refusing to mark any spool `Spooled`.
    ///
    /// The run stops at the one point that leaves a spool file plus a row
    /// naming it, with the row still saying the file may be half-written.
    pub(crate) fn refusing_to_mark_spooled(inner: &'a dyn Index) -> Self {
        Self {
            inner,
            spooling: AtomicUsize::new(0),
            refuse_mark_spooled: true,
        }
    }

    /// How many spools were announced before their files existed.
    pub(crate) fn spooling_rows(&self) -> usize {
        self.spooling.load(Ordering::Relaxed)
    }
}

/// What the refused marking reports.
///
/// A backend fault, which is the shape of this failure that says nothing is
/// wrong with the Container: the spool on disk is whole and the catalog's own
/// store is what would not write it down.
fn refused() -> IndexError {
    IndexError::Backend {
        operation: "marking a Container spooled",
        cause: Box::new(std::io::Error::other(
            "the catalog was interrupted before the spool was recorded as complete",
        )),
    }
}

#[async_trait]
impl Index for WatchingIndex<'_> {
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

    /// Holds a Spooling row to the ordering the spool steps promise, then
    /// records it.
    ///
    /// A panic rather than a returned error: a row written after its file is not
    /// a failure the flow could report and recover from, it is the ordering
    /// invariant broken, and the case that drove the run is what has to fail.
    async fn record_pending_upload(&self, pending: PendingUpload) -> IndexResult<()> {
        if pending.state == SpoolState::Spooling {
            assert!(
                !tokio::fs::try_exists(&pending.spool_path)
                    .await
                    .expect("asking whether a spool file exists must succeed"),
                "a Spooling row must be recorded before its spool file exists, \
                 and this one names a file that is already there",
            );
            assert!(
                pending.object_ref.is_none(),
                "a Container is uploaded only out of a finished spool, so a Spooling \
                 row can carry no object handle",
            );
            self.spooling.fetch_add(1, Ordering::Relaxed);
        }
        self.inner.record_pending_upload(pending).await
    }

    async fn mark_spooled(&self, container_id: ContainerId) -> IndexResult<()> {
        if self.refuse_mark_spooled {
            return Err(refused());
        }
        self.inner.mark_spooled(container_id).await
    }

    async fn clear_pending_upload(&self, container_id: ContainerId) -> IndexResult<()> {
        self.inner.clear_pending_upload(container_id).await
    }

    async fn pending_uploads(&self) -> IndexResult<Vec<PendingUpload>> {
        self.inner.pending_uploads().await
    }
}
