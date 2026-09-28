use super::AddedFile;

/// What one mapped folder holds on this device that the Library does not, one
/// level down — the answer of
/// [`added_locally`](crate::OpenLibrary::added_locally).
///
/// Files and folders are answered for differently, because they are different
/// things to a person looking at the folder. A file is a row: it has a size and
/// a time, it opens in the reader, and the next run carries it in. A folder is
/// only a name. What is under it is that folder's own answer when somebody
/// opens it, and saying it is there is enough for the one question it is here
/// for — whether a place is already taken on disk, so that a folder made there
/// in the explorer is not made over files somebody already has (spec: PK-17).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalAdditions {
    /// The files standing in the mapped folder that no Entry stands at, in
    /// EP-3 order.
    pub files: Vec<AddedFile>,
    /// The names of the folders standing in the mapped folder that are not
    /// folders of the Library — no Entry is under them — in EP-3 order.
    pub folders: Vec<String>,
}

impl LocalAdditions {
    /// Whether the mapped folder holds nothing the Library does not.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.folders.is_empty()
    }
}
