use std::fmt;

use coffret_model::{ContainerId, EntryLocation, EntryNumber, EntryRename};

/// One Entry, named by what no rename changes: the Container holding it and
/// its entry number there (spec: FM-9).
///
/// A batch's pending changes name the Entries they act on this way, so a
/// rebase can apply what intervening records did to those Entries without
/// asking what they were called: a record's renames name the Entry by the
/// same pair (spec: FM-15, EP-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryRef {
    /// The Container holding the Entry.
    pub container_id: ContainerId,
    /// The Entry's position in that Container's entry table.
    pub entry_number: EntryNumber,
}

impl EntryRef {
    /// The Entry numbered `entry_number` in `container_id`.
    pub const fn new(container_id: ContainerId, entry_number: EntryNumber) -> Self {
        Self {
            container_id,
            entry_number,
        }
    }

    /// The Entry a catalog location stands for.
    pub const fn of(location: &EntryLocation) -> Self {
        Self::new(location.container_id, location.entry_number)
    }

    /// The Entry one element of a record's `renames` names.
    pub const fn renamed_by(rename: &EntryRename) -> Self {
        Self::new(rename.container_id(), rename.entry_number())
    }
}

impl fmt::Display for EntryRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Entry {} of Container {}",
            self.entry_number, self.container_id
        )
    }
}
