use coffret_model::EntryPath;

/// A file a freeze selects whose addition would leave one Entry Path both a
/// file and a folder: it stands under a path an Entry stands at, or Entries
/// stand under its own path (spec: EP-4, EP-6).
///
/// A sync leaves such a file out and carries on with the rest. A freeze does
/// not: the Pack it would build is one book, and a file left out of it could
/// never be put into it once it is committed — only beside it, as an Entry of
/// its own, or by deleting the Pack and freezing again. So one of these stops
/// the whole run before anything is packed, and a run, or a preview, names
/// every one of them (spec: EP-6, PK-17).
///
/// The Entry Paths travel in the value because the caller is what tells a
/// person what to rename. They never travel into a diagnostic event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAndFolder {
    /// Where in the Library the file would stand.
    pub path: EntryPath,
    /// The path that would be both a file and a folder: a folder above
    /// `path`, or `path` itself.
    pub both: EntryPath,
    /// Whether `both` is the name a top-level mapping of this device is
    /// mapped at (spec: EP-9).
    ///
    /// The folder of the two is then that mapping's own, and its place in the
    /// Library is the name it was mapped at rather than its local folder's:
    /// renaming that folder on disk changes nothing, and what helps is
    /// renaming the file.
    pub mapped: bool,
}
