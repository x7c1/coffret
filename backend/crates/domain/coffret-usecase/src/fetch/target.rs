use coffret_model::{EntryLocation, EntryPath};

use crate::fetch::local_place::LocalPlace;

/// One current Entry, and the place on this device a mapping translates it to.
///
/// The pair is what a mapping produces and nothing more: where the Entry is in
/// the Library, and where its file would go here (spec: EP-9). It says nothing
/// yet about whether the file is there or may be written, which is the next
/// step's question (spec: EP-10, EP-11).
#[derive(Debug, Clone)]
pub(super) struct Target {
    /// The Entry, and the Container holding it (spec: CP-11).
    pub(super) location: EntryLocation,
    /// Where on this device the Entry's file belongs.
    ///
    /// A [`LocalPlace`] rather than a path, because the two steps after this one
    /// write there: the mapped root and the components below it stay apart so
    /// that both the look and the write descend them rather than joining them
    /// (spec: EP-4, EP-11).
    ///
    /// Device state and nothing else: it never travels into a Container, a
    /// Journal record, or a log line.
    pub(super) place: LocalPlace,
}

impl Target {
    /// The Library position this target stands at.
    pub(super) fn path(&self) -> &EntryPath {
        self.location.path()
    }
}
