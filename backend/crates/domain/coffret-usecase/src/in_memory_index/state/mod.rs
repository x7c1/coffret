use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{
    ContainerId, ContainerSummary, ControlObjectName, EntryLocation, EntryPath, IndexCheckpoint,
    SnapshotContent,
};

use crate::device_state::{
    AttemptedSlot, BatchId, DeviceTime, HeldParcel, LocalEntry, LocalEntryState, LocalObservation,
    Mapping, PendingRow, SpoolState,
};
use crate::index_error::{IndexError, IndexResult};

mod apply;

mod restore;

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

    // Device-local: never uploaded, and touched by the operations above only
    // in which Entry a file is marked to follow (spec: FM-15, EP-10).
    // Keyed by prefix, and holding the whole mapping: one per prefix is what the
    // port promises (spec: EP-9), and the local root is no longer the only thing
    // a mapping carries (spec: EP-12).
    mappings: BTreeMap<Option<EntryPath>, Mapping>,
    local_entries: BTreeMap<EntryPath, LocalEntry>,
    pending_rows: BTreeMap<ContainerId, PendingRow>,
    /// The commit slots each batch with pending rows attempted (spec: OC-2),
    /// ordered the way the port answers for them.
    attempted_slots: BTreeMap<BatchId, BTreeSet<AttemptedSlot>>,
    /// The parcels this device holds, keyed and so ordered the way the port
    /// answers for them (spec: PK-21).
    held_parcels: BTreeMap<(ContainerId, u64), HeldParcel>,
}

impl State {
    /// The Library-wide half, in the canonical order the maps already hold it
    /// in (spec: CK-8, EP-3).
    pub(super) fn snapshot(&self) -> IndexResult<SnapshotContent> {
        let checkpoint = self.checkpoint.clone().ok_or(IndexError::NoCheckpoint)?;
        // The maps iterate in the canonical order already, so this is the one
        // construction that has nothing to sort — but it is still built through
        // the constructor, which is what says the content holds together.
        SnapshotContent::new(
            checkpoint,
            self.adopted_from.clone(),
            self.containers.values().cloned().collect(),
            self.entries.values().cloned().collect(),
        )
        .map_err(|cause| IndexError::UnreadableCatalog {
            operation: "reading the Library-wide state",
            cause: std::sync::Arc::new(cause),
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

    pub(super) fn entries_in(&self, container_id: ContainerId) -> Vec<EntryLocation> {
        let mut held: Vec<EntryLocation> = self
            .entries
            .values()
            .filter(|entry| entry.container_id == container_id)
            .cloned()
            .collect();
        held.sort_by_key(|entry| entry.entry_number);
        held
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

    /// Stores a mapping as given, its recorded filesystem identity included: a
    /// mapping recorded afresh with none clears the one that was there, which is
    /// how a device re-confirms a root a run reported unavailable
    /// (spec: EP-12).
    pub(super) fn set_mapping(&mut self, mapping: Mapping) {
        self.mappings.insert(mapping.prefix.clone(), mapping);
    }

    pub(super) fn mappings(&self) -> impl Iterator<Item = &Mapping> {
        self.mappings.values()
    }

    pub(super) fn mark_present(&mut self, observation: LocalObservation) {
        self.local_entries.insert(
            observation.path.clone(),
            LocalEntry {
                observation,
                state: LocalEntryState::Present,
                relocate_to: None,
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

    pub(super) fn forget_local_entry(&mut self, path: &EntryPath) {
        self.local_entries.remove(path);
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

    pub(super) fn record_pending_row(&mut self, pending: PendingRow) {
        self.pending_rows.insert(pending.container_id, pending);
    }

    /// Marks one spool file whole, and a Container with no row changes nothing:
    /// inventing one would record a spool the flow never announced.
    pub(super) fn mark_spooled(&mut self, container_id: ContainerId) {
        // A row that is already whole keeps what it says, the object handle
        // included, the way the SQLite adapter's `UPDATE` of the state alone
        // leaves the handle where it was.
        if let Some(pending) = self.pending_rows.get_mut(&container_id) {
            if pending.state == SpoolState::Spooling {
                pending.state = SpoolState::Spooled(None);
            }
        }
    }

    /// Marks the rows of a batch whose commit is about to be sent into `slot`,
    /// and records the slot against the batch of every row it marks, all under
    /// the one lock, so no reader sees part of the batch marked or a marked row
    /// without its slot. A Container with no row changes nothing.
    pub(super) fn mark_commit_attempted(
        &mut self,
        container_ids: &[ContainerId],
        slot: &AttemptedSlot,
    ) {
        for container_id in container_ids {
            if let Some(pending) = self.pending_rows.get_mut(container_id) {
                pending.commit_attempted = true;
                self.attempted_slots
                    .entry(pending.batch.clone())
                    .or_default()
                    .insert(slot.clone());
            }
        }
    }

    pub(super) fn attempted_slots(&self, batch: &BatchId) -> Vec<AttemptedSlot> {
        self.attempted_slots
            .get(batch)
            .map(|slots| slots.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Drops one row, and the slots of its batch with the batch's last row.
    pub(super) fn clear_pending_row(&mut self, container_id: ContainerId) {
        let Some(removed) = self.pending_rows.remove(&container_id) else {
            return;
        };
        if !self
            .pending_rows
            .values()
            .any(|pending| pending.batch == removed.batch)
        {
            self.attempted_slots.remove(&removed.batch);
        }
    }

    pub(super) fn pending_rows(&self) -> Vec<PendingRow> {
        self.pending_rows.values().cloned().collect()
    }

    pub(super) fn hold_parcel(&mut self, parcel: HeldParcel) {
        self.held_parcels
            .insert((parcel.container_id, parcel.index), parcel);
    }

    pub(super) fn held_parcels(&self) -> Vec<HeldParcel> {
        self.held_parcels.values().cloned().collect()
    }

    pub(super) fn let_go_parcel(&mut self, container_id: ContainerId, index: u64) {
        self.held_parcels.remove(&(container_id, index));
    }

    /// Ends the wait of the file at `path`, whose Entry has left the Library
    /// (spec: EP-10, EP-15).
    ///
    /// Where no current Entry stands at that path the row stays, following
    /// nothing, and the file is departed there. Where one does, it is another
    /// Entry than the one the file is a copy of, so the row is forgotten: a
    /// record kept there would claim that Entry present and offer this file's
    /// bytes as its edit. Forgotten, the file is one this device never placed
    /// for it (spec: EP-11).
    fn end_wait(&mut self, path: &EntryPath) {
        if self.entries.contains_key(path) {
            self.local_entries.remove(path);
        } else if let Some(local) = self.local_entries.get_mut(path) {
            local.relocate_to = None;
        }
    }

    /// The rows of files this device has on disk as copies of the Entries at
    /// their own paths: a file waiting to follow a rename elsewhere is neither
    /// (spec: EP-10).
    fn present(&self) -> impl Iterator<Item = &LocalEntry> {
        self.local_entries
            .values()
            .filter(|local| local.state == LocalEntryState::Present && !local.awaits_relocation())
    }
}

/// Whether `path` is covered by a prefix, `None` being the Library root and
/// covering everything.
///
/// Where the subtree itself ends is [`EntryPath::is_under`]'s to say, so that
/// this catalog, a scan narrowing to a mapping, and a fetch narrowing to a
/// subtree cannot disagree about it (spec: EP-2, EP-9).
fn is_under(path: &EntryPath, prefix: Option<&EntryPath>) -> bool {
    prefix.is_none_or(|prefix| path.is_under(prefix))
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
