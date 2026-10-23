use async_trait::async_trait;
use coffret_model::{
    ContainerId, ContainerSummary, EntryLocation, EntryPath, IndexCheckpoint, JournalRecord,
    SnapshotContent,
};

use crate::committed_batch::CommittedBatch;
use crate::device_state::{
    AttemptedSlot, BatchId, DeviceTime, LocalEntry, LocalObservation, Mapping, PendingRow,
};
use crate::index::Index;
use crate::index_error::{IndexError, IndexResult};

/// A catalog that refuses the one write only the committer can do — or the
/// one a relocation makes after its file is relocated — wrapped around the real
/// one.
///
/// [`Index::refresh`] is the post-commit step whose failure leaves the device
/// out of step with a Library that has already changed, and that state cannot be
/// reached by driving the flow: the record has to land and the refresh has to
/// fail, in that order, inside one call. Setting the state up by hand afterwards
/// would test something else — what is being checked is that the run stops there
/// and that what it leaves behind is enough for the next one to finish the job.
///
/// [`Index::relocate_local_entries`] is the same shape on a relocation's side:
/// the file is relocated on disk and its record has not followed, which only a
/// catalog failing between the two arranges (spec: EP-10).
///
/// Everything else is passed straight through to the catalog the backend handed
/// the suite, which is the same catalog the run afterwards uses directly: what
/// the interrupted run wrote down really is what the next one finds.
pub(crate) struct RefusingIndex<'a> {
    inner: &'a dyn Index,
    refused: Refused,
}

/// Which write the catalog refuses.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Refused {
    Refresh,
    Relocation,
}

impl<'a> RefusingIndex<'a> {
    /// Refuses every [`refresh`](Index::refresh) and answers everything else
    /// honestly.
    pub(crate) fn around(inner: &'a dyn Index) -> Self {
        Self {
            inner,
            refused: Refused::Refresh,
        }
    }

    /// Refuses every [`relocate_local_entries`](Index::relocate_local_entries)
    /// and answers everything else honestly.
    pub(crate) fn refusing_relocations(inner: &'a dyn Index) -> Self {
        Self {
            inner,
            refused: Refused::Relocation,
        }
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
        cause: std::sync::Arc::new(std::io::Error::other(
            "the catalog was interrupted after the batch's record landed",
        )),
    }
}

#[async_trait]
impl Index for RefusingIndex<'_> {
    async fn own_pending_rows(&self) -> IndexResult<crate::PendingRowsGuard> {
        self.inner.own_pending_rows().await
    }

    async fn restore(&self, snapshot: SnapshotContent) -> IndexResult<()> {
        self.inner.restore(snapshot).await
    }

    async fn apply(&self, record: JournalRecord) -> IndexResult<()> {
        self.inner.apply(record).await
    }

    async fn refresh(&self, batch: CommittedBatch) -> IndexResult<()> {
        match self.refused {
            Refused::Refresh => Err(refused()),
            Refused::Relocation => self.inner.refresh(batch).await,
        }
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

    async fn entries_in(&self, container_id: ContainerId) -> IndexResult<Vec<EntryLocation>> {
        self.inner.entries_in(container_id).await
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

    async fn forget_local_entry(&self, path: &EntryPath) -> IndexResult<()> {
        self.inner.forget_local_entry(path).await
    }

    async fn local_entry_at(&self, path: &EntryPath) -> IndexResult<Option<LocalEntry>> {
        self.inner.local_entry_at(path).await
    }

    async fn present_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<LocalEntry>> {
        self.inner.present_under(prefix).await
    }

    async fn entries_absent_under(
        &self,
        prefix: Option<&EntryPath>,
    ) -> IndexResult<Vec<EntryLocation>> {
        self.inner.entries_absent_under(prefix).await
    }

    async fn present_without_entry(&self) -> IndexResult<Vec<LocalEntry>> {
        self.inner.present_without_entry().await
    }

    async fn awaiting_relocation(&self) -> IndexResult<Vec<LocalEntry>> {
        self.inner.awaiting_relocation().await
    }

    async fn relocate_local_entries(
        &self,
        relocated: &[(EntryPath, EntryPath)],
    ) -> IndexResult<()> {
        match self.refused {
            Refused::Relocation => Err(refused()),
            Refused::Refresh => self.inner.relocate_local_entries(relocated).await,
        }
    }

    async fn record_pending_row(&self, pending: PendingRow) -> IndexResult<()> {
        self.inner.record_pending_row(pending).await
    }

    async fn mark_spooled(&self, container_id: ContainerId) -> IndexResult<()> {
        self.inner.mark_spooled(container_id).await
    }

    async fn mark_commit_attempted(
        &self,
        container_ids: &[ContainerId],
        slot: &AttemptedSlot,
    ) -> IndexResult<()> {
        self.inner.mark_commit_attempted(container_ids, slot).await
    }

    async fn attempted_slots(&self, batch: &BatchId) -> IndexResult<Vec<AttemptedSlot>> {
        self.inner.attempted_slots(batch).await
    }

    async fn clear_pending_row(&self, container_id: ContainerId) -> IndexResult<()> {
        self.inner.clear_pending_row(container_id).await
    }

    async fn pending_rows(&self) -> IndexResult<Vec<PendingRow>> {
        self.inner.pending_rows().await
    }

    async fn hold_parcel(&self, parcel: crate::device_state::HeldParcel) -> IndexResult<()> {
        self.inner.hold_parcel(parcel).await
    }

    async fn held_parcels(&self) -> IndexResult<Vec<crate::device_state::HeldParcel>> {
        self.inner.held_parcels().await
    }

    async fn let_go_parcel(&self, container_id: ContainerId, index: u64) -> IndexResult<()> {
        self.inner.let_go_parcel(container_id, index).await
    }

    async fn reserve(
        &self,
        folder: Option<&EntryPath>,
    ) -> IndexResult<crate::device_state::ReservationId> {
        self.inner.reserve(folder).await
    }

    async fn extend_reservation(
        &self,
        id: crate::device_state::ReservationId,
        path: &EntryPath,
    ) -> IndexResult<()> {
        self.inner.extend_reservation(id, path).await
    }

    async fn reservations(&self) -> IndexResult<Vec<crate::device_state::Reservation>> {
        self.inner.reservations().await
    }

    async fn release_reservation(&self, id: crate::device_state::ReservationId) -> IndexResult<()> {
        self.inner.release_reservation(id).await
    }
}
