use std::collections::BTreeSet;

use coffret_device::{DeleteSelection, EntryPath};

/// What one deletion is asked to take out of the Library: folders with
/// everything under them, individual files, or both (spec: PK-9, EP-9).
///
/// It names Entries and never Containers. Which Containers go and which are
/// rebuilt around what they keep is worked out from the catalog when the run
/// starts, because one folder can touch any number of Packs (spec: PK-8).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Target {
    /// Folders to delete, each with everything under it.
    ///
    /// Never the Library root: a deletion of the root's whole folder would be a
    /// deletion of the whole Library, and the routes refuse it before a
    /// `Target` is made.
    pub folders: BTreeSet<EntryPath>,
    /// Individual files to delete.
    pub paths: BTreeSet<EntryPath>,
}

impl Target {
    /// What the use case is asked to delete.
    pub fn selection(&self) -> DeleteSelection {
        DeleteSelection {
            paths: self.paths.clone(),
            folders: self.folders.clone(),
        }
    }

    /// Whether it names nothing, which on the wire is the Library root.
    pub fn is_whole_library(&self) -> bool {
        self.folders.is_empty() && self.paths.is_empty()
    }

    /// The folders, as the wire spells them, in Entry Path order.
    pub fn folders_named(&self) -> Vec<String> {
        self.folders
            .iter()
            .map(|folder| folder.as_str().to_owned())
            .collect()
    }

    /// The files, as the wire spells them, in Entry Path order.
    pub fn paths_named(&self) -> Vec<String> {
        self.paths
            .iter()
            .map(|path| path.as_str().to_owned())
            .collect()
    }
}
