use std::path::PathBuf;

use coffret_model::{EntryLocation, EntryPath};

/// One current Entry, and the local path a mapping translates it to.
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
    /// Device state and nothing else: it never travels into a Container, a
    /// Journal record, or a log line.
    pub(super) local_path: PathBuf,
}

impl Target {
    /// The Library position this target stands at.
    pub(super) fn path(&self) -> &EntryPath {
        self.location.path()
    }
}
