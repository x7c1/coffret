use std::collections::BTreeMap;

use coffret_model::{EntryLocation, EntryPath};

use super::State;
use crate::entry_ref::EntryRef;

impl State {
    /// Brings the rows at `paths` in line with where the catalog holds the
    /// Entry each of them is a copy of (spec: EP-10, FM-15).
    ///
    /// A row whose Entry is current waits to follow it where it stands at
    /// another path, and waits for nothing where it stands at the row's own.
    /// A row whose Entry has left the Library waits for nothing either, and
    /// where no current Entry stands at its path it stays, and the file is
    /// departed there (spec: EP-15). Where one does, it is another Entry than
    /// the one the file is a copy of, so the row is forgotten: a record kept
    /// there would claim that Entry present and offer this file's bytes as its
    /// edit. Forgotten, the file is one this device never placed for it
    /// (spec: EP-11).
    pub(super) fn reconcile(&mut self, paths: impl IntoIterator<Item = EntryPath>) {
        let standing: BTreeMap<EntryRef, &EntryPath> = self
            .entries
            .values()
            .map(|entry| (EntryRef::of(entry), entry.path()))
            .collect();
        for path in paths {
            let Some(local) = self.local_entries.get_mut(&path) else {
                continue;
            };
            match standing.get(&local.observation.entry) {
                Some(&at) => local.relocate_to = (*at != path).then(|| at.clone()),
                None if self.entries.contains_key(&path) => {
                    self.local_entries.remove(&path);
                }
                None => local.relocate_to = None,
            }
        }
    }

    /// [`reconcile`](Self::reconcile) over every row this device holds.
    pub(super) fn reconcile_all(&mut self) {
        let paths: Vec<EntryPath> = self.local_entries.keys().cloned().collect();
        self.reconcile(paths);
    }

    /// Where the Entry of each row standing for one of `held` stands now,
    /// read before the catalog changes: the row's own path and that one
    /// (spec: EP-6).
    pub(super) fn standing_of(
        &self,
        held: impl Fn(&EntryRef) -> bool,
    ) -> Vec<(EntryPath, EntryPath)> {
        let standing: BTreeMap<EntryRef, &EntryPath> = self
            .entries
            .values()
            .map(|entry| (EntryRef::of(entry), entry.path()))
            .collect();
        self.local_entries
            .iter()
            .filter(|(_, local)| held(&local.observation.entry))
            .filter_map(|(path, local)| {
                standing
                    .get(&local.observation.entry)
                    .map(|&at| (path.clone(), at.clone()))
            })
            .collect()
    }

    /// Carries each row in `stood` over to the Entry now standing where its
    /// own stood, where `replaces` says that Entry takes its place
    /// (spec: EP-6, EP-10).
    pub(super) fn carry_over(
        &mut self,
        stood: &[(EntryPath, EntryPath)],
        replaces: impl Fn(&EntryLocation) -> bool,
    ) {
        for (path, stood_at) in stood {
            let Some(successor) = self.entries.get(stood_at).filter(|entry| replaces(entry)) else {
                continue;
            };
            if let Some(local) = self.local_entries.get_mut(path) {
                local.observation.entry = EntryRef::of(successor);
            }
        }
    }
}
