use coffret_model::EntryPath;

/// The files on either side of one row of the Library, in reading order: the
/// one a reader steps back onto and the one it reads on into.
///
/// Reading order is the order the explorer's tree draws the whole Library in
/// with every folder open: depth first, and in each folder its folders before
/// its files, each in EP-3 order. Folders take no place of their own in it, so
/// the files either side of a row may stand in any folder, at any depth — the
/// last file of one folder is followed by the first file of the next.
///
/// Only the Library's own Entries are in it. A file standing in a mapped
/// folder that no run has carried in is not in the catalog, which is the only
/// thing this is read out of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesBeside {
    /// The file before the row, or `None` where the row is at the start of
    /// the Library.
    pub previous: Option<EntryPath>,
    /// The file after it, or `None` where the row is at the end.
    pub next: Option<EntryPath>,
}
