use std::sync::Mutex;

use async_trait::async_trait;
use coffret_model::{
    ContainerId, ContainerSummary, EntryLocation, EntryPath, IndexCheckpoint, JournalRecord,
    SnapshotContent,
};

use crate::committed_batch::CommittedBatch;
use crate::device_state::{DeviceTime, LocalEntry, LocalObservation, Mapping, PendingUpload};
use crate::index::Index;
use crate::index_error::IndexResult;

mod state;
use state::State;

/// An [`Index`] kept in memory, for the contract suite and for cases that need
/// a catalog without a file.
///
/// It is the reference reading of the port: every operation is what the trait's
/// documentation says and nothing more, so a case that fails against an adapter
/// but passes here is the adapter's disagreement rather than the case's.
///
/// A whole operation is applied to a copy of the state and installed at the
/// end, so a rejected replay leaves the catalog as it was — the atomicity the
/// port promises, without a transaction to get it from.
#[derive(Debug, Default)]
pub struct InMemoryIndex {
    state: Mutex<State>,
}

impl InMemoryIndex {
    /// A catalog standing at no committed Library state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs `change` against a copy and installs it only if it succeeded.
    fn write<T>(&self, change: impl FnOnce(&mut State) -> IndexResult<T>) -> IndexResult<T> {
        let mut state = self.locked();
        let mut draft = state.clone();
        let outcome = change(&mut draft)?;
        *state = draft;
        Ok(outcome)
    }

    fn locked(&self) -> std::sync::MutexGuard<'_, State> {
        // A caller that panicked mid-operation left the copy uninstalled, so
        // what is behind the lock is still a whole state.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl Index for InMemoryIndex {
    async fn restore(&self, snapshot: SnapshotContent) -> IndexResult<()> {
        self.write(|state| state.restore(snapshot))
    }

    async fn apply(&self, record: JournalRecord) -> IndexResult<()> {
        self.write(|state| state.apply(record))
    }

    async fn refresh(&self, batch: CommittedBatch) -> IndexResult<()> {
        self.write(|state| state.refresh(batch))
    }

    async fn snapshot(&self) -> IndexResult<SnapshotContent> {
        self.locked().snapshot()
    }

    async fn checkpoint(&self) -> IndexResult<Option<IndexCheckpoint>> {
        Ok(self.locked().checkpoint())
    }

    async fn entry_at(&self, path: &EntryPath) -> IndexResult<Option<EntryLocation>> {
        Ok(self.locked().entry_at(path))
    }

    async fn entries_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<EntryLocation>> {
        Ok(self.locked().entries_under(prefix))
    }

    async fn containers_under(
        &self,
        prefix: Option<&EntryPath>,
    ) -> IndexResult<Vec<ContainerSummary>> {
        Ok(self.locked().containers_under(prefix))
    }

    async fn set_mapping(&self, mapping: Mapping) -> IndexResult<()> {
        self.locked()
            .set_mapping(mapping.prefix, mapping.local_root);
        Ok(())
    }

    async fn mappings(&self) -> IndexResult<Vec<Mapping>> {
        Ok(self
            .locked()
            .mappings()
            .map(|(prefix, local_root)| Mapping {
                prefix: prefix.clone(),
                local_root: local_root.clone(),
            })
            .collect())
    }

    async fn mark_present(&self, observation: LocalObservation) -> IndexResult<()> {
        self.locked().mark_present(observation);
        Ok(())
    }

    async fn mark_absent(&self, path: &EntryPath, at: DeviceTime) -> IndexResult<()> {
        self.locked().mark_absent(path, at);
        Ok(())
    }

    async fn local_entry_at(&self, path: &EntryPath) -> IndexResult<Option<LocalEntry>> {
        Ok(self.locked().local_entry_at(path))
    }

    async fn present_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<LocalEntry>> {
        Ok(self.locked().present_under(prefix))
    }

    async fn present_without_entry(&self) -> IndexResult<Vec<LocalEntry>> {
        Ok(self.locked().present_without_entry())
    }

    async fn record_pending_upload(&self, pending: PendingUpload) -> IndexResult<()> {
        self.locked().record_pending_upload(pending);
        Ok(())
    }

    async fn complete_pending_spool(&self, container_id: ContainerId) -> IndexResult<()> {
        self.locked().complete_pending_spool(container_id);
        Ok(())
    }

    async fn clear_pending_upload(&self, container_id: ContainerId) -> IndexResult<()> {
        self.locked().clear_pending_upload(container_id);
        Ok(())
    }

    async fn pending_uploads(&self) -> IndexResult<Vec<PendingUpload>> {
        Ok(self.locked().pending_uploads())
    }
}
