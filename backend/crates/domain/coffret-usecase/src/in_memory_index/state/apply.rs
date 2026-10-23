use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, EntryLocation, EntryNumber, JournalRecord};

use super::{insert_container, insert_entry, State};
use crate::catalog_renames::{self, Named, RenamedEntry};
use crate::committed_batch::CommittedBatch;
use crate::device_state::{LocalObservation, ReservationId};
use crate::index_error::{IndexError, IndexResult};

impl State {
    /// Replays one committed Journal record (spec: CP-1, CP-11, EP-6, FM-15).
    ///
    /// Runs against the draft [`InMemoryIndex`](crate::InMemoryIndex) installs
    /// only on success, so a refused rename takes nothing else of the record
    /// with it.
    pub(in crate::in_memory_index) fn apply(&mut self, record: JournalRecord) -> IndexResult<()> {
        // A record at or below the head this catalog stands at is one it holds
        // already, refused before any of it is applied (spec: CK-9, RV-5).
        if let Some(at) = &self.checkpoint {
            if record.generation() <= at.head_generation() {
                return Err(IndexError::AlreadyApplied {
                    generation: record.generation(),
                    checkpoint: at.head_generation(),
                });
            }
        }
        // Removals leave first: a path may pass from a replaced Container to
        // its replacement within one record (spec: EP-6).
        let checkpoint = record.checkpoint();
        let added: BTreeSet<ContainerId> = record
            .additions()
            .iter()
            .map(|addition| addition.container().id)
            .collect();
        // Where the Entries of the rows this record's removals reach stand,
        // read before they are taken out: a replacement takes a row over at
        // the path its Entry stood at (spec: EP-6, EP-10).
        let removed = record.removals();
        let stood = self.standing_of(|entry| removed.contains(&entry.container_id));
        for removed in record.removals() {
            self.containers.remove(removed);
            self.entries
                .retain(|_, entry| entry.container_id != *removed);
        }
        // Then every rename at once, against what the removals left
        // (spec: FM-15).
        let renamed = self.check_renames(&record)?;
        for one in &renamed {
            self.entries.remove(one.from());
        }
        for one in &renamed {
            insert_entry(&mut self.entries, &self.containers, one.after())?;
        }
        for addition in record.into_additions() {
            // Each Entry keeps the number its place in the table gives it
            // (spec: FM-9, FM-15).
            let (container, located) = addition.into_locations();
            insert_container(&mut self.containers, container)?;
            for entry in located {
                insert_entry(&mut self.entries, &self.containers, entry)?;
            }
        }
        self.carry_over(&stood, |entry| added.contains(&entry.container_id));
        // A record touches few rows, but a catalog in memory is a case's own,
        // so every row is brought in line rather than only the ones it
        // reached.
        self.reconcile_all();
        self.checkpoint = Some(checkpoint);
        Ok(())
    }

    /// Checks `record`'s renames against the catalog as it stands.
    fn check_renames(&self, record: &JournalRecord) -> IndexResult<Vec<RenamedEntry>> {
        if record.renames().is_empty() {
            return Ok(Vec::new());
        }
        // One pass over the catalog answers every element by its name rather
        // than one pass per element.
        let by_number: BTreeMap<(ContainerId, EntryNumber), &EntryLocation> = self
            .entries
            .values()
            .map(|entry| ((entry.container_id, entry.entry_number), entry))
            .collect();
        catalog_renames::check_renames(
            record,
            |container_id, entry_number| {
                Ok(match by_number.get(&(container_id, entry_number)) {
                    Some(entry) => Named::Entry(Box::new((*entry).clone())),
                    None if self.containers.contains_key(&container_id) => Named::NoEntry,
                    None => Named::NoContainer,
                })
            },
            |path| {
                Ok(self
                    .entries
                    .get(path)
                    .map(|entry| (entry.container_id, entry.entry_number)))
            },
        )
    }

    /// Applies this device's own committed batch (spec: CP-1, EP-10, OC-2,
    /// PK-17).
    pub(in crate::in_memory_index) fn refresh(&mut self, batch: CommittedBatch) -> IndexResult<()> {
        let CommittedBatch {
            record,
            materialized,
            released,
        } = batch;
        let uploaded = uploaded(&record);
        self.apply(record)?;
        self.complete(&uploaded, materialized, released);
        Ok(())
    }

    /// Records the device-local half of this device's own batch, whose record the
    /// catalog holds already (spec: EP-10, OC-2, PK-17).
    pub(in crate::in_memory_index) fn complete_applied(&mut self, batch: CommittedBatch) {
        self.complete(&uploaded(&batch.record), batch.materialized, batch.released);
    }

    /// The device-local half of a refresh: the files it put on disk become
    /// `present` (spec: EP-10), its spools stop being pending (spec: OC-2),
    /// and the reservations its batch packs are released (spec: PK-17).
    fn complete(
        &mut self,
        uploaded: &[ContainerId],
        materialized: Vec<LocalObservation>,
        released: Vec<ReservationId>,
    ) {
        for observation in materialized {
            self.mark_present(observation);
        }
        for &container_id in uploaded {
            self.clear_pending_row(container_id);
        }
        for id in released {
            self.release_reservation(id);
        }
    }
}

/// The Containers a record adds, whose pending rows its commit clears.
fn uploaded(record: &JournalRecord) -> Vec<ContainerId> {
    record
        .additions()
        .iter()
        .map(|addition| addition.container().id)
        .collect()
}
