use coffret_model::EntryPath;

use super::{ChildFolder, FileRow};

/// What one folder of the Library holds, one level down.
///
/// A Library has no folders of its own: an Entry Path is one string and `/` is
/// the only logical separator in it (spec: EP-2), so a folder is what the
/// separator implies rather than something the catalog stores. This is that
/// implication made explicit for one folder — the child folders and the child
/// files — and nothing under them.
///
/// Both lists are in EP-3 order: the byte order of the canonical paths, with no
/// case folding and no locale. That is the order the catalog answered in, and it
/// is the order because it is the only one every device agrees on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderListing {
    /// The folder this is a listing of, or `None` for the Library root.
    pub path: Option<EntryPath>,
    /// The folders directly inside it.
    pub folders: Vec<ChildFolder>,
    /// The files directly inside it.
    pub files: Vec<FileRow>,
}
