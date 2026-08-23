use std::collections::BTreeMap;
use std::path::PathBuf;

use coffret_model::{
    ContainerId, ContainerSummary, ControlObjectName, EntryLocation, EntryPath, IndexCheckpoint,
};

use crate::committed_batch::CommittedBatch;
use crate::device_state::{
    DeviceTime, LocalEntry, LocalEntryState, LocalObservation, PendingUpload,
};
use crate::index_error::{IndexError, IndexResult};
use crate::journal_record::JournalRecord;
use crate::snapshot_content::SnapshotContent;

/// Everything one in-memory Index holds.
///
/// The maps are ordered, which is what makes the canonical order the port
/// promises fall out of iteration rather than out of a sort: `ContainerId`
/// orders by its bytes and `EntryPath` by the canonical UTF-8 of the path
/// (spec: EP-3).
#[derive(Debug, Default, Clone)]
pub(super) struct State {
    // Library-wide: exactly what an Index Snapshot carries (spec: CK-7).
    checkpoint: Option<IndexCheckpoint>,
    adopted_from: Option<ControlObjectName>,
    containers: BTreeMap<ContainerId, ContainerSummary>,
    entries: BTreeMap<EntryPath, EntryLocation>,

    // Device-local: never uploaded, never touched by the operations above.
    mappings: BTreeMap<Option<EntryPath>, PathBuf>,
    local_entries: BTreeMap<EntryPath, LocalEntry>,
    pending_uploads: BTreeMap<ContainerId, PendingUpload>,
}

impl State {
    /// Replaces the Library-wide half with a Snapshot's content, leaving device
    /// state alone (spec: CK-7, RV-1).
    pub(super) fn restore(&mut self, snapshot: SnapshotContent) -> IndexResult<()> {
        let mut containers = BTreeMap::new();
        for container in snapshot.containers {
            insert_container(&mut containers, container)?;
        }
        let mut entries = BTreeMap::new();
        for entry in snapshot.entries {
            insert_entry(&mut entries, &containers, entry)?;
        }
        self.containers = containers;
        self.entries = entries;
        self.checkpoint = Some(snapshot.checkpoint);
        self.adopted_from = snapshot.adopted_from;
        Ok(())
    }

    /// Replays one committed Journal record (spec: CP-1, CP-11, EP-6).
    pub(super) fn apply(&mut self, record: JournalRecord) -> IndexResult<()> {
        // Removals leave first: a path may move from a replaced Container to
        // its replacement within one record (spec: EP-6).
        for removed in &record.removals {
            self.containers.remove(removed);
            self.entries
                .retain(|_, entry| entry.container_id != *removed);
        }
        for addition in record.additions {
            let container_id = addition.container.id;
            insert_container(&mut self.containers, addition.container)?;
            for entry in addition.entries {
                insert_entry(
                    &mut self.entries,
                    &self.containers,
                    EntryLocation {
                        container_id,
                        entry,
                    },
                )?;
            }
        }
        self.checkpoint = Some(IndexCheckpoint {
            master_key_epoch: record.master_key_epoch,
            head_generation: record.generation,
            journal_generation: record.generation,
            next_commit_slot: record.next_commit_slot,
            keyring: record.keyring,
        });
        Ok(())
    }

    /// Applies this device's own committed batch (spec: CP-1, EP-10, OC-2).
    pub(super) fn refresh(&mut self, batch: CommittedBatch) -> IndexResult<()> {
        let uploaded: Vec<ContainerId> = batch
            .record
            .additions
            .iter()
            .map(|addition| addition.container.id)
            .collect();

        self.apply(batch.record)?;

        for observation in batch.materialized {
            self.mark_present(observation);
        }
        for container_id in uploaded {
            self.pending_uploads.remove(&container_id);
        }
        Ok(())
    }

    /// The Library-wide half, in the canonical order the maps already hold it
    /// in (spec: CK-8, EP-3).
    pub(super) fn snapshot(&self) -> IndexResult<SnapshotContent> {
        let checkpoint = self.checkpoint.clone().ok_or(IndexError::NoCheckpoint)?;
        Ok(SnapshotContent {
            checkpoint,
            adopted_from: self.adopted_from.clone(),
            containers: self.containers.values().cloned().collect(),
            entries: self.entries.values().cloned().collect(),
        })
    }

    pub(super) fn checkpoint(&self) -> Option<IndexCheckpoint> {
        self.checkpoint.clone()
    }

    pub(super) fn entry_at(&self, path: &EntryPath) -> Option<EntryLocation> {
        self.entries.get(path).cloned()
    }

    pub(super) fn entries_under(&self, prefix: Option<&EntryPath>) -> Vec<EntryLocation> {
        self.entries
            .values()
            .filter(|entry| is_under(entry.path(), prefix))
            .cloned()
            .collect()
    }

    pub(super) fn containers_under(&self, prefix: Option<&EntryPath>) -> Vec<ContainerSummary> {
        let mut held: Vec<ContainerId> = self
            .entries
            .values()
            .filter(|entry| is_under(entry.path(), prefix))
            .map(|entry| entry.container_id)
            .collect();

        // Distinct: Packs from different `freeze` invocations may overlap, so
        // one Container can hold many Entries under one prefix (spec: PK-8).
        held.sort_unstable();
        held.dedup();
        held.iter()
            .filter_map(|id| self.containers.get(id).cloned())
            .collect()
    }

    pub(super) fn set_mapping(&mut self, prefix: Option<EntryPath>, local_root: PathBuf) {
        self.mappings.insert(prefix, local_root);
    }

    pub(super) fn mappings(&self) -> impl Iterator<Item = (&Option<EntryPath>, &PathBuf)> {
        self.mappings.iter()
    }

    pub(super) fn mark_present(&mut self, observation: LocalObservation) {
        self.local_entries.insert(
            observation.path.clone(),
            LocalEntry {
                observation,
                state: LocalEntryState::Present,
            },
        );
    }

    /// Marks a file this device had as gone.
    ///
    /// A path with no row is left alone rather than given one: only an Entry
    /// this device materialized can be absent, and inventing a row would make
    /// the scan claim a deletion the device never witnessed (spec: EP-10).
    pub(super) fn mark_absent(&mut self, path: &EntryPath, at: DeviceTime) {
        if let Some(local) = self.local_entries.get_mut(path) {
            local.state = LocalEntryState::Absent;
            local.observation.at = at;
        }
    }

    pub(super) fn local_entry_at(&self, path: &EntryPath) -> Option<LocalEntry> {
        self.local_entries.get(path).cloned()
    }

    pub(super) fn present_under(&self, prefix: Option<&EntryPath>) -> Vec<LocalEntry> {
        self.present()
            .filter(|local| is_under(&local.observation.path, prefix))
            .cloned()
            .collect()
    }

    pub(super) fn present_without_entry(&self) -> Vec<LocalEntry> {
        self.present()
            .filter(|local| !self.entries.contains_key(&local.observation.path))
            .cloned()
            .collect()
    }

    pub(super) fn record_pending_upload(&mut self, pending: PendingUpload) {
        self.pending_uploads.insert(pending.container_id, pending);
    }

    pub(super) fn clear_pending_upload(&mut self, container_id: ContainerId) {
        self.pending_uploads.remove(&container_id);
    }

    pub(super) fn pending_uploads(&self) -> Vec<PendingUpload> {
        self.pending_uploads.values().cloned().collect()
    }

    fn present(&self) -> impl Iterator<Item = &LocalEntry> {
        self.local_entries
            .values()
            .filter(|local| local.state == LocalEntryState::Present)
    }
}

/// Whether `path` is the prefix itself or lies beneath it.
///
/// `None` is the Library root and covers everything. A prefix covers the Entry
/// at exactly that path and everything under `prefix/`, and nothing else:
/// `books` never covers `books-annex/page-1.png`, because the only logical
/// separator is `/` (spec: EP-2, EP-9).
fn is_under(path: &EntryPath, prefix: Option<&EntryPath>) -> bool {
    let Some(prefix) = prefix else {
        return true;
    };
    let (path, prefix) = (path.as_str(), prefix.as_str());
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn insert_container(
    containers: &mut BTreeMap<ContainerId, ContainerSummary>,
    container: ContainerSummary,
) -> IndexResult<()> {
    let container_id = container.id;
    if containers.insert(container_id, container).is_some() {
        return Err(IndexError::DuplicateContainer { container_id });
    }
    Ok(())
}

fn insert_entry(
    entries: &mut BTreeMap<EntryPath, EntryLocation>,
    containers: &BTreeMap<ContainerId, ContainerSummary>,
    entry: EntryLocation,
) -> IndexResult<()> {
    if !containers.contains_key(&entry.container_id) {
        return Err(IndexError::UnknownContainer {
            container_id: entry.container_id,
        });
    }
    let path = entry.path().clone();
    if entries.insert(path.clone(), entry).is_some() {
        return Err(IndexError::DuplicatePath { path });
    }
    Ok(())
}
