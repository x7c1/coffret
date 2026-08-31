use coffret_model::EntryPath;

/// One folder inside another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildFolder {
    /// Its last path component, which is what it is called.
    pub name: String,
    /// Where in the Library it stands.
    pub path: EntryPath,
    /// Whether a mapping of this device reaches it (spec: EP-9), as
    /// [`FolderListing::mapped`](super::FolderListing::mapped) means it.
    ///
    /// In the row because a child can differ from the folder above it: mappings
    /// are made at the top level, so the children of the Library root are the
    /// one place two siblings can have different answers. Deeper down every
    /// child shares its parent's, and the field repeats that answer rather than
    /// leaving a reader to work out which case it is in.
    pub mapped: bool,
}
