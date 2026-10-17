use crate::container_id::ContainerId;
use crate::entry_number::EntryNumber;
use crate::entry_path::EntryPath;
use crate::error::{Error, Result};

/// One element of a Journal record's `renames`: the Entry a commit gives a new
/// current Entry Path (spec: FM-15).
///
/// The Entry is named by `(Container ID, entry number)` rather than by its
/// current path, because the name is what changes and the Container never does
/// (spec: FM-9). The old name is not carried: it is the catalog's to know, and
/// a second copy of it here would be a second statement that could disagree.
///
/// What the element alone can show is held here: the new path is an Entry Path
/// (by its type, spec: EP-1, EP-2) and it does not lie in the namespace coffret
/// reserves for derived Entries. Whether the Container is current, whether the
/// number lies inside its table, whether the Entry is a derived one, and
/// whether the path changes anything are questions about the catalog the
/// record is applied to, and are asked where it is applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRename {
    container_id: ContainerId,
    entry_number: EntryNumber,
    path: EntryPath,
}

impl EntryRename {
    /// The rename of the Entry numbered `entry_number` in `container_id` to
    /// `path`.
    ///
    /// # Errors
    ///
    /// [`Error::RenameIntoReservedNamespace`] where `path` lies under the
    /// top-level component reserved for derived Entries: derived Entries are
    /// not renamed by this version, and an Entry renamed into their namespace
    /// would become one (spec: FM-15).
    pub fn new(
        container_id: ContainerId,
        entry_number: EntryNumber,
        path: EntryPath,
    ) -> Result<Self> {
        if path.is_derived() {
            return Err(Error::RenameIntoReservedNamespace);
        }
        Ok(Self {
            container_id,
            entry_number,
            path,
        })
    }

    /// The Container holding the renamed Entry.
    pub const fn container_id(&self) -> ContainerId {
        self.container_id
    }

    /// The renamed Entry's position in that Container's entry table.
    pub const fn entry_number(&self) -> EntryNumber {
        self.entry_number
    }

    /// The Entry Path the Entry stands at once the record is applied.
    pub const fn path(&self) -> &EntryPath {
        &self.path
    }

    /// The key `renames` is ordered by: the Container ID's sixteen bytes, then
    /// the entry number (spec: FM-15).
    pub fn order_key(&self) -> (ContainerId, EntryNumber) {
        (self.container_id, self.entry_number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{container_id, entry_path};

    // FM-15: the reserved top-level component is refused as a rename's target,
    // both as a folder and as the name itself, and nothing that merely starts
    // with the same letters is.
    #[test]
    fn a_rename_into_the_derived_namespace_cannot_exist() {
        for path in [".coffret-derived/thumb/a.jpg", ".coffret-derived"] {
            assert!(
                matches!(
                    EntryRename::new(container_id(1), EntryNumber::FIRST, entry_path(path)),
                    Err(Error::RenameIntoReservedNamespace)
                ),
                "{path} is refused",
            );
        }
        let rename = EntryRename::new(
            container_id(1),
            EntryNumber::FIRST,
            entry_path(".coffret-derived-not/a.jpg"),
        )
        .expect("a neighbouring name is an ordinary path");
        assert_eq!(rename.path().as_str(), ".coffret-derived-not/a.jpg");
    }
}
