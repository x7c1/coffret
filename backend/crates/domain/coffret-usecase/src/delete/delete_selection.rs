use std::collections::BTreeSet;

use coffret_model::EntryPath;

/// Which Entries a deletion takes out of the Library (spec: PK-9).
///
/// Two ways of naming them, and a request may use both: individual Entry
/// Paths, each naming the one current Entry at that path (spec: EP-5), and a
/// folder, naming the current Entry at that path, if any, and every one
/// beneath it — the subtree a mapping covers (spec: EP-9). The Entries the
/// selection names are the union of the two.
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
    /// A folder to delete, with everything under it.
    pub folder: Option<EntryPath>,
}

impl DeleteSelection {
    /// A selection of exactly these Entry Paths.
    pub fn paths(paths: BTreeSet<EntryPath>) -> Self {
        Self {
            paths,
            folder: None,
        }
    }

    /// A selection of one folder and everything under it.
    pub fn folder(folder: EntryPath) -> Self {
        Self {
            paths: BTreeSet::new(),
            folder: Some(folder),
        }
    }

    /// The same selection, also naming a folder and everything under it.
    pub fn and_folder(mut self, folder: EntryPath) -> Self {
        self.folder = Some(folder);
        self
    }

    /// Whether it names nothing at all.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.folder.is_none()
    }
}
