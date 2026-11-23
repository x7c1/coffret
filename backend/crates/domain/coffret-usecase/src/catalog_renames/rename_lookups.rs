use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, EntryLocation, EntryNumber, EntryPath, EntryRename};

use super::Named;
use crate::entry_ref::EntryRef;
use crate::index::Index;
use crate::index_error::IndexResult;

/// What [`check_rename_set`](super::check_rename_set) asks of the catalog about
/// one set of renames, looked up before it is asked (spec: FM-15, RV-5).
///
/// The check asks two things and nothing else: which Entry each element names,
/// and which Entry stands at each element's destination. So those are read
/// off the catalog — every current Entry of each Container a rename names, and
/// the Entry at each destination — rather than the Library, and the check's
/// two lookups are answered from them. A Container holding no current Entry
/// is no current Container (spec: FM-10), which is how an element naming one
/// is told apart from one naming a number past a current one's table.
pub(crate) struct RenameLookups {
    /// Every current Entry of the Containers the renames name.
    named: BTreeMap<EntryRef, EntryLocation>,
    /// Those of the Containers the renames name that are current.
    containers: BTreeSet<ContainerId>,
    /// The Entry standing at each destination, where one does.
    standing: BTreeMap<EntryPath, EntryRef>,
}

impl RenameLookups {
    /// Looks up what checking `renames` asks, in the catalog as it stands.
    pub(crate) async fn read(index: &dyn Index, renames: &[EntryRename]) -> IndexResult<Self> {
        let mut named = BTreeMap::new();
        let mut containers = BTreeSet::new();
        let asked: BTreeSet<ContainerId> = renames.iter().map(EntryRename::container_id).collect();
        for container_id in asked {
            for location in index.entries_in(container_id).await? {
                containers.insert(container_id);
                named.insert(EntryRef::of(&location), location);
            }
        }
        let mut standing = BTreeMap::new();
        for rename in renames {
            if let Some(location) = index.entry_at(rename.path()).await? {
                standing.insert(rename.path().clone(), EntryRef::of(&location));
            }
        }
        Ok(Self {
            named,
            containers,
            standing,
        })
    }

    /// Which Entry `(container_id, entry_number)` names, as the check's first
    /// lookup asks.
    pub(crate) fn named(&self, container_id: ContainerId, entry_number: EntryNumber) -> Named {
        match self.named.get(&EntryRef::new(container_id, entry_number)) {
            Some(location) => Named::Entry(Box::new(location.clone())),
            None if self.containers.contains(&container_id) => Named::NoEntry,
            None => Named::NoContainer,
        }
    }

    /// The Entry standing at `path`, a destination of the renames, as the
    /// check's second lookup asks.
    pub(crate) fn standing_at(&self, path: &EntryPath) -> Option<EntryRef> {
        self.standing.get(path).copied()
    }
}
