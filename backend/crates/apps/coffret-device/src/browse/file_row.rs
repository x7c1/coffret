use coffret_model::{ContainerKind, EntryPath, Mtime};

use super::EntryState;

/// One Entry as a row in a listing.
///
/// Everything here is independent of how it might be displayed. There is no
/// thumbnail, no dimension, no page count and no media type: those are either
/// derived Entries the Library does not hold yet or a reader's own reading of a
/// name, and a row that carried them would be a row that has to be rebuilt every
/// time a reader learns a new format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRow {
    /// Its last path component, which is what it is called.
    pub name: String,
    /// Where in the Library it stands.
    pub path: EntryPath,
    /// The Entry's plaintext length in bytes.
    pub size: u64,
    /// The file's own modification time, as the Container preserved it
    /// (spec: FM-9).
    pub mtime: Mtime,
    /// Whether this device has the file (spec: EP-10).
    pub state: EntryState,
    /// Whether the Entry lives in a Container of its own or inside a Pack
    /// (spec: PK-15).
    ///
    /// In the row rather than asked for separately because of what the answer
    /// decides: an Entry inside a Pack cannot be replaced one file at a time,
    /// so whatever offers to write over it has to know before it offers.
    pub container: ContainerKind,
}
