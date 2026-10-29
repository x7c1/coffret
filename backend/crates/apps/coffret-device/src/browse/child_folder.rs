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
    /// Whether this device has every Entry at every depth below this folder
    /// (spec: EP-10): the promise that the whole book, or the whole series,
    /// reads without reaching Storage.
    ///
    /// Answered here rather than left to a browser because a browser holds the
    /// listings it has asked for and nothing deeper, and this needs every
    /// descendant. The listing's own range scan already reads the whole subtree
    /// and which of it is present, so the answer costs nothing more.
    ///
    /// Only a present materialization record counts. A file whose fetch failed
    /// left none, and a file this device witnessed go had its record turned
    /// absent, so either keeps the folder from being present throughout. A folder
    /// of the Library always has an Entry under it (spec: EP-2), so `true` is
    /// never said of nothing.
    ///
    /// Not what [`OpenLibrary::wholly_present`](crate::OpenLibrary::wholly_present)
    /// answers: that asks only of the Entries standing directly in a folder,
    /// which is what a fill needs, and says nothing of the folders below it.
    pub present_throughout: bool,
}
