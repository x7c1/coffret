use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, SnapshotContent};

use super::{insert_container, insert_entry, State};
use crate::entry_ref::EntryRef;
use crate::index_error::IndexResult;

impl State {
    /// Replaces the Library-wide half with a Snapshot's content, leaving device
    /// state alone save for what each record says about its Entry
    /// (spec: CK-7, RV-1, FM-15).
    pub(in crate::in_memory_index) fn restore(
        &mut self,
        snapshot: SnapshotContent,
    ) -> IndexResult<()> {
        let library_name = snapshot.library_name().cloned();
        let (checkpoint, adopted_from, held, located) = snapshot.into_parts();
        let mut containers = BTreeMap::new();
        for container in held {
            insert_container(&mut containers, container)?;
        }
        let mut entries = BTreeMap::new();
        for entry in located {
            insert_entry(&mut entries, &containers, entry)?;
        }
        // Where each record's Entry stands in the catalog being replaced, and
        // which Containers that catalog held: what tells a replacement from an
        // Entry that was already there (spec: EP-6).
        let stood = self.standing_of(|_| true);
        let replaced: BTreeSet<ContainerId> = self.containers.keys().copied().collect();
        self.containers = containers;
        self.entries = entries;
        self.checkpoint = Some(checkpoint);
        self.adopted_from = adopted_from;
        self.library_name = library_name;

        // A record whose Entry the Snapshot still holds stays with it, wherever
        // it stands now. One whose Entry is gone is carried over to the Entry
        // standing where its own stood, but only where that Entry's Container
        // is new: a replacement always brings one, and an Entry that was
        // current already — one a rename brought there — is another Entry
        // than the one the file is a copy of (spec: EP-6, FM-15).
        let held: BTreeSet<EntryRef> = self.entries.values().map(EntryRef::of).collect();
        let gone: Vec<_> = stood
            .into_iter()
            .filter(|(path, _)| {
                self.local_entries
                    .get(path)
                    .is_some_and(|local| !held.contains(&local.observation.entry))
            })
            .collect();
        self.carry_over(&gone, |entry| !replaced.contains(&entry.container_id));
        self.reconcile_all();
        Ok(())
    }
}
