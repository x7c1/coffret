use crate::container_id::ContainerId;
use crate::deposited_at::DepositedAt;
use crate::entry_extent::EntryExtent;
use crate::entry_metadata::EntryMetadata;
use crate::entry_number::EntryNumber;
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
    /// The Entry's position in that Container's entry table: with
    /// `container_id`, the name for this Entry that a rename leaves
    /// unchanged (spec: FM-9).
    pub entry_number: EntryNumber,
    /// What that Container's entry table records about it (spec: FM-9).
    pub entry: EntryMetadata,
    /// When the content this Entry holds was deposited into the Library
    /// (spec: FM-15, FM-16).
    ///
    /// The catalog's own value and none of the Container's: no meta section
    /// carries it, and a rename or a rewrite that carries this content into
    /// another Entry carries the time with it.
    pub deposited_at: DepositedAt,
}

impl EntryLocation {
    /// The Library position this Entry occupies.
    pub fn path(&self) -> &EntryPath {
        &self.entry.path
    }

    /// Where this Entry's plaintext lies in its Container's plaintext stream —
    /// what says which parcels of a Pack a reader of this Entry asks for
    /// (spec: PK-16, PK-19).
    pub fn extent(&self) -> EntryExtent {
        self.entry.extent
    }
}
