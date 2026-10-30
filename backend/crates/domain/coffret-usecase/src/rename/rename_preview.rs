use coffret_model::EntryPath;

use crate::rename::rename_refusal::RenameRefusal;
use crate::rename::renamed_path::RenamedPath;

/// What a rename of a set of pairs would do, worked out without writing
/// anything — the answer of [`preview_rename`](super::preview_rename).
///
/// The run's own plan over the catalog caught up to the head, and what this
/// device's own files of the renamed Entries would do once it lands
/// (spec: FM-15, EP-10). A run made afterwards can differ from it only where
/// the Library moves in between, or where a rename the preview counted as one
/// that follows is refused by the disk as crossing filesystems.
#[derive(Debug, Clone, Default)]
pub struct RenamePreview {
    /// Every Entry the rename would give a new path, ordered by the path it
    /// leaves; empty where the set is refused.
    pub renamed: Vec<RenamedPath>,
    /// Why the set is refused as a whole, and `None` where the plan admits it.
    pub refused: Option<RenameRefusal>,
    /// How many of this device's files of those Entries would follow them
    /// with one rename (spec: EP-10).
    pub following: usize,
    /// How many of them could not follow, and would leave this device: no
    /// mapped folder of this device reaches the new path, or the one that does
    /// stands on another filesystem (spec: EP-10).
    pub leaving: usize,
    /// Their total length on disk, in bytes.
    pub leaving_bytes: u64,
    /// This device's files of those Entries that changed since it placed them
    /// (spec: EP-15), which a relocation keeps where they stand rather than
    /// moving or discarding.
    pub edited: Vec<EntryPath>,
}
