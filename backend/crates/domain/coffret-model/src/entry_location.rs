use crate::container_id::ContainerId;
use crate::entry_extent::EntryExtent;
use crate::entry_metadata::EntryMetadata;
use crate::entry_path::EntryPath;

/// Where the Entry currently at one Entry Path lives.
///
/// This is the mapping the Index exists for: an Entry Path answered with the
/// Container that holds it and the Entry's place inside that Container, so that
/// opening a file needs no lookup on Storage and no Container is opened to find
/// out what another one holds (spec: RV-5).
///
/// At every committed Library state one Entry Path identifies at most one
/// current Entry, so a location is the whole answer for a path rather than one
/// of several (spec: EP-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryLocation {
    /// The Container holding this Entry.
    pub container_id: ContainerId,
    /// What that Container's entry table records about it (spec: FM-9).
    pub entry: EntryMetadata,
}

impl EntryLocation {
    /// The Library position this Entry occupies.
    pub fn path(&self) -> &EntryPath {
        &self.entry.path
    }

    /// Where this Entry's plaintext lies in its Container's plaintext stream —
    /// what a range read of a single Entry out of a Pack is aimed with
    /// (spec: PK-16).
    pub fn extent(&self) -> EntryExtent {
        self.entry.extent
    }
}
