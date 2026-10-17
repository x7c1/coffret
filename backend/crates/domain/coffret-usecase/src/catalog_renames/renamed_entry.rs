use coffret_model::{EntryLocation, EntryPath};

/// One rename a record makes, checked against the catalog it is applied to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamedEntry {
    /// The Entry as it stands before the record.
    pub before: EntryLocation,
    /// The Entry Path it stands at once the record is applied.
    pub path: EntryPath,
}

impl RenamedEntry {
    /// The Entry Path the Entry leaves.
    pub fn from(&self) -> &EntryPath {
        self.before.path()
    }

    /// The Entry as it stands once the record is applied.
    ///
    /// Only its current path changes. Its captured path is its Container's
    /// record of its name and no rename rewrites a Container, so the catalog
    /// carries the captured path apart from the current one exactly where the
    /// two now differ — renaming an Entry back to its captured path makes them
    /// equal again (spec: FM-9, FM-15).
    pub fn after(&self) -> EntryLocation {
        let mut after = self.before.clone();
        after.entry.path = self.path.clone();
        after
    }
}
