use std::sync::Mutex;

use async_trait::async_trait;
use coffret_model::{
    ContainerId, ContainerSummary, EntryLocation, EntryPath, Generation, IndexCheckpoint,
    JournalRecord, SnapshotContent,
};

use crate::committed_batch::CommittedBatch;
use crate::device_state::{DeviceTime, LocalEntry, LocalObservation, Mapping, PendingUpload};
use crate::index::Index;
use crate::index_error::{IndexError, IndexResult};

/// A catalog with a second replayer standing over it, wrapped around the real
/// one.
///
/// The situation it puts a catch-up in is the ordinary arrangement rather than a
/// fault: one catalog file, two processes holding the Library open — a server
/// answering a browser while a `sync` runs in a terminal — each having listed
/// the Journal from its own reading of the checkpoint, so both replay the same
/// records. It cannot be reached by driving the flow twice, because whether the
/// two overlap at all depends on the runtime and on how fast Storage answers.
/// Here the overlap is put exactly where the rule is: the rival lands its
/// records *inside* the call in which this catalog is asked to apply the first
/// of them, so the refusal happens on every backend rather than on the slow
/// ones.
///
/// Everything else is passed straight through to the catalog the backend handed
/// the suite, which is the same catalog the case afterwards reads: what the
/// contended replay left behind really is what a device would find.
pub(super) struct RivalIndex<'a> {
    inner: &'a dyn Index,
    /// The records the rival replays, the moment this catalog is asked for the
    /// first of them. Emptied once, so the rival replays once.
    ahead: Mutex<Vec<JournalRecord>>,
    /// The record whose replay is refused without the catalog moving at all.
    unexplained_at: Option<Generation>,
}

impl<'a> RivalIndex<'a> {
    /// A rival that replays `ahead` — which must be in generation order — as
    /// soon as this catalog reaches the first of them.
    ///
    /// The rival's records land through the same catalog, so what the wrapped
    /// call then meets is the port's own refusal over a checkpoint that has
    /// genuinely moved, and not a refusal this fixture invented.
    pub(super) fn racing(inner: &'a dyn Index, ahead: Vec<JournalRecord>) -> Self {
        Self {
            inner,
            ahead: Mutex::new(ahead),
            unexplained_at: None,
        }
    }

    /// A catalog that refuses one record the way a duplicate reads, while
    /// standing exactly where it did.
    ///
    /// The other half of the rule: the refusal looks like the one a rival
    /// produces and nothing has happened to explain it, so it is a Library state
    /// no commit could have produced (spec: EP-5) and the catch-up owes the
    /// caller the refusal rather than a convergence.
    pub(super) fn refusing_without_moving(inner: &'a dyn Index, at: Generation) -> Self {
        Self {
            inner,
            ahead: Mutex::new(Vec::new()),
            unexplained_at: Some(at),
        }
    }

    /// The rival's records, if this is the call it gets in ahead of.
    fn ahead_of(&self, generation: Generation) -> Vec<JournalRecord> {
        let mut held = self
            .ahead
            .lock()
            .expect("the rival's records are only ever taken, never left poisoned");
        match held.first() {
            Some(first) if first.generation == generation => std::mem::take(&mut held),
            _ => Vec::new(),
        }
    }
}

/// What a record already in the catalog collides on, as the catalog would say
/// it (spec: EP-5).
fn duplicate(record: &JournalRecord) -> IndexError {
    let path = record
        .additions
        .first()
        .and_then(|addition| addition.entries.first())
        .map(|entry| entry.path.clone())
        .expect("a case that refuses a record gives it an Entry to collide on");

    IndexError::DuplicatePath { path }
}

#[async_trait]
impl Index for RivalIndex<'_> {
    async fn restore(&self, snapshot: SnapshotContent) -> IndexResult<()> {
        self.inner.restore(snapshot).await
    }

    async fn apply(&self, record: JournalRecord) -> IndexResult<()> {
        if self.unexplained_at == Some(record.generation) {
            return Err(duplicate(&record));
        }
        for rival in self.ahead_of(record.generation) {
            self.inner
                .apply(rival)
                .await
                .expect("the rival replays records this catalog has not seen");
        }
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
