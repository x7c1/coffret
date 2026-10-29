use std::collections::BTreeSet;

use coffret_model::EntryPath;

/// Which Entries a deletion takes out of the Library (spec: PK-9).
///
/// Two ways of naming them, and a request may use both: individual Entry
/// Paths, each naming the one current Entry at that path (spec: EP-5), and
/// folders, each naming the current Entry at that path, if any, and every one
/// beneath it — the subtree a mapping covers (spec: EP-9). The Entries the
/// selection names are the union of all of them, so a folder inside another
/// named folder, or a file inside one, adds nothing.
///
/// A selection names Entries and never Containers: which Containers go, and
/// which are rebuilt around the Entries left in them, is worked out from the
/// catalog, because Pack path ranges overlap and interleave and one folder can
/// touch any number of Packs (spec: PK-8, PK-9).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeleteSelection {
    /// Individual Entry Paths to delete.
    ///
    /// A path that holds no current Entry deletes nothing, and is reported
    /// rather than passed over: a person who named a file and is told it is
    /// gone should not be told so of a file the Library never held.
    pub paths: BTreeSet<EntryPath>,
    /// Folders to delete, each with everything under it.
    ///
    /// Several in one selection so that the Packs they share are rebuilt once
    /// rather than once per folder — the volumes of one series packed
    /// together, say.
    pub folders: BTreeSet<EntryPath>,
}

impl DeleteSelection {
    /// A selection of exactly these Entry Paths.
    pub fn paths(paths: BTreeSet<EntryPath>) -> Self {
        Self {
            paths,
            folders: BTreeSet::new(),
        }
    }

    /// A selection of one folder and everything under it.
    pub fn folder(folder: EntryPath) -> Self {
        Self {
            paths: BTreeSet::new(),
            folders: BTreeSet::from([folder]),
        }
    }

    /// The same selection, also naming a folder and everything under it.
    pub fn and_folder(mut self, folder: EntryPath) -> Self {
        self.folders.insert(folder);
        self
    }

    /// Whether it names nothing at all.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.folders.is_empty()
    }
}
