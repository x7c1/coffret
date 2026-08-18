use crate::container_id::ContainerId;
use crate::entry_path::EntryPath;

/// Points at the Entry that derived data — a thumbnail, a transcode — was
/// produced from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DerivedFrom {
    /// The Container holding the parent Entry.
    pub container_id: ContainerId,
    /// The parent Entry's path.
    pub path: EntryPath,
}
