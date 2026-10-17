use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, EntryLocation, EntryNumber, EntryPath, JournalRecord};

use super::{insert_container, insert_entry, State};
use crate::catalog_renames::{self, Named, RenamedEntry};
use crate::committed_batch::CommittedBatch;
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
        let waiting = self.following_any_of(record.removals());
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
        self.follow_renames(&renamed);
        for addition in record.into_additions() {
            // Each Entry keeps the number its place in the table gives it
            // (spec: FM-9, FM-15).
            let (container, located) = addition.into_locations();
            insert_container(&mut self.containers, container)?;
            for entry in located {
                insert_entry(&mut self.entries, &self.containers, entry)?;
            }
        }
        self.release_vacated(&waiting, &added);
        self.checkpoint = Some(checkpoint);
        Ok(())
    }

    /// The files waiting to follow an Entry of one of `removed`, read before a
    /// record takes those Containers out (spec: EP-10, FM-15).
    ///
    /// Read off the rows rather than the paths the removals vacate: a rename of
    /// the same record may give such a path to an Entry a file already follows
    /// there, and that file's wait is not this record's to end.
    fn following_any_of(&self, removed: &[ContainerId]) -> Vec<EntryPath> {
        if removed.is_empty() {
            return Vec::new();
        }
        self.local_entries
            .iter()
            .filter_map(|(path, local)| {
                let to = local.relocate_to.as_ref()?;
                self.entries
                    .get(to)
                    .is_some_and(|entry| removed.contains(&entry.container_id))
                    .then(|| path.clone())
            })
            .collect()
    }

    /// Ends the wait of each file in `waiting`, whose Entry this record's
    /// removals took out, unless the path it waited for passed to a Container
    /// the record adds in its place (spec: EP-6, EP-10, FM-15).
    fn release_vacated(&mut self, waiting: &[EntryPath], added: &BTreeSet<ContainerId>) {
        for path in waiting {
            let Some(to) = self
                .local_entries
                .get(path)
                .and_then(|local| local.relocate_to.as_ref())
            else {
                continue;
            };
            let replaced = self
                .entries
                .get(to)
                .is_some_and(|entry| added.contains(&entry.container_id));
            if !replaced {
                self.end_wait(path);
            }
        }
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

    /// Marks the rows of the files this device has of renamed Entries as
    /// waiting to follow them (spec: EP-10, FM-15).
    ///
    /// A file stays where it was placed, and so does its row. A row follows the
    /// Entry it is a copy of: the Entry at its own path where it follows no
    /// earlier rename, or the Entry an earlier rename sent it after. Every
    /// rename is matched against the rows as they were, so two Entries swapping
    /// their names swap what their two files wait for, and a file whose Entry
    /// is renamed back to where it stands waits for nothing any more.
    fn follow_renames(&mut self, renamed: &[RenamedEntry]) {
        if renamed.is_empty() {
            return;
        }
        let to_by_from: BTreeMap<&EntryPath, &EntryPath> =
            renamed.iter().map(|one| (one.from(), &one.path)).collect();
        let following: Vec<(EntryPath, EntryPath)> = self
            .local_entries
            .iter()
            .filter_map(|(path, local)| {
                let copy_of = local.relocate_to.as_ref().unwrap_or(path);
                to_by_from
                    .get(copy_of)
                    .map(|to| (path.clone(), (*to).clone()))
            })
            .collect();
        for (path, to) in following {
            if let Some(local) = self.local_entries.get_mut(&path) {
                local.relocate_to = (to != path).then_some(to);
            }
        }
    }

    /// Applies this device's own committed batch (spec: CP-1, EP-10, OC-2).
    pub(in crate::in_memory_index) fn refresh(&mut self, batch: CommittedBatch) -> IndexResult<()> {
        let uploaded: Vec<ContainerId> = batch
            .record
            .additions()
            .iter()
            .map(|addition| addition.container().id)
            .collect();

        self.apply(batch.record)?;

        for observation in batch.materialized {
            self.mark_present(observation);
        }
        for container_id in uploaded {
            self.clear_pending_row(container_id);
        }
        Ok(())
    }
}
