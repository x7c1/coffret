use coffret_model::{EntryPath, Mtime};

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
    /// Not what [`OpenLibrary::directly_present`](crate::OpenLibrary::directly_present)
    /// answers: that asks only of the Entries standing directly in a folder,
    /// which is what a fill needs, and says nothing of the folders below it.
    ///
    /// The same answer as [`files_present`](Self::files_present) reaching
    /// [`files_total`](Self::files_total), kept as a field of its own because
    /// it is the promise a reader acts on, and spelling it once here keeps
    /// every caller from comparing the counts itself.
    pub present_throughout: bool,
    /// How many Entries stand at every depth below this folder: the files of
    /// the whole subtree, not the rows one level down.
    ///
    /// Never zero, for the reason `true` above is never said of nothing: a
    /// folder of the Library always has an Entry under it (spec: EP-2).
    pub files_total: u64,
    /// How many of those [`files_total`](Self::files_total) this device has,
    /// by the same present materialization records `present_throughout`
    /// reads (spec: EP-10).
    ///
    /// A count and not only the yes-or-no, because "18 of 196 files" is what a
    /// person deciding whether to fetch the rest of a book needs, and only the
    /// catalog sees every descendant to count them. The listing's own range
    /// scan already reads them, so the count costs nothing more.
    pub files_present: u64,
    /// The plaintext bytes of those [`files_total`](Self::files_total) Entries
    /// together, whether this device has them or not: what the folder comes
    /// to in the Library, as a desktop file manager states a folder's size.
    pub size: u64,
    /// The latest modification time among those
    /// [`files_total`](Self::files_total) Entries, at any depth below this
    /// folder (spec: FM-9): when anything in it last changed.
    ///
    /// A folder of the Library has no time of its own to report — it is what
    /// the paths under it imply (spec: EP-2), not a thing the catalog records
    /// — so its Modified is what a reader asking "when did this book last
    /// change" means: the newest file anywhere inside it. Always some Entry's
    /// time, since a folder always has an Entry under it.
    pub mtime: Mtime,
}
