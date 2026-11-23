use super::{FolderFile, FolderSummary};

/// What one folder holds, one level down: its child folders and the Entries
/// standing directly in it, each list in EP-3 order (spec: EP-3).
///
/// An Entry standing at exactly the folder's own path is in neither: it is not
/// a child of itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderContents {
    /// The folders directly inside it, with what each comes to.
    pub folders: Vec<FolderSummary>,
    /// The Entries directly inside it.
    pub files: Vec<FolderFile>,
}
