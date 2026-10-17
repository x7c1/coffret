use std::collections::BTreeMap;

use coffret_model::{ContainerId, EntryNumber, EntryPath, SnapshotContent};

use super::{insert_container, insert_entry, State};
use crate::index_error::IndexResult;

impl State {
    /// Replaces the Library-wide half with a Snapshot's content, leaving device
    /// state alone save for which Entry each local file follows
    /// (spec: CK-7, RV-1, FM-15).
    pub(in crate::in_memory_index) fn restore(
        &mut self,
        snapshot: SnapshotContent,
    ) -> IndexResult<()> {
        let (checkpoint, adopted_from, held, located) = snapshot.into_parts();
        let mut containers = BTreeMap::new();
        for container in held {
            insert_container(&mut containers, container)?;
        }
        let mut entries = BTreeMap::new();
        for entry in located {
            insert_entry(&mut entries, &containers, entry)?;
        }
        // Which Entry each file is a copy of, by the name no rename changes,
        // read off the catalog being replaced.
        let followed: Vec<(EntryPath, (ContainerId, EntryNumber))> = self
            .local_entries
            .iter()
            .filter_map(|(path, local)| {
                let copy_of = local.relocate_to.as_ref().unwrap_or(path);
                self.entries
                    .get(copy_of)
                    .map(|entry| (path.clone(), (entry.container_id, entry.entry_number)))
            })
            .collect();
        self.containers = containers;
        self.entries = entries;
        self.checkpoint = Some(checkpoint);
        self.adopted_from = adopted_from;
        self.follow_adopted(&followed);
        Ok(())
    }

    /// Marks each file whose Entry the adopted Snapshot holds at another path
    /// to follow it there, as the replay of the renames in between would have
    /// (spec: CK-9, FM-15).
    ///
    /// An Entry the Snapshot no longer holds leaves its file following what
    /// stands at the path it followed, if anything does, the way a replay
    /// leaves the file of a replaced Entry (spec: EP-6, EP-10).
    fn follow_adopted(&mut self, followed: &[(EntryPath, (ContainerId, EntryNumber))]) {
        let by_number: BTreeMap<(ContainerId, EntryNumber), &EntryPath> = self
            .entries
            .values()
            .map(|entry| ((entry.container_id, entry.entry_number), entry.path()))
            .collect();
        let relocations: Vec<(EntryPath, EntryPath)> = followed
            .iter()
            .filter_map(|(path, name)| by_number.get(name).map(|to| (path.clone(), (*to).clone())))
            .collect();
        for (path, to) in relocations {
            if let Some(local) = self.local_entries.get_mut(&path) {
                local.relocate_to = (to != path).then_some(to);
            }
        }
        self.drop_stale_relocations();
    }

    /// Ends the wait of every file whose Entry has left the catalog, rather
    /// than leaving it waiting on a path that holds nothing it is a copy of
    /// (spec: EP-10, EP-15).
    fn drop_stale_relocations(&mut self) {
        let stale: Vec<EntryPath> = self
            .local_entries
            .iter()
            .filter(|(_, local)| {
                local
                    .relocate_to
                    .as_ref()
                    .is_some_and(|to| !self.entries.contains_key(to))
            })
            .map(|(path, _)| path.clone())
            .collect();
        for path in &stale {
            self.end_wait(path);
        }
    }
}
