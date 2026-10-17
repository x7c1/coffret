use coffret_format::ContainerOutline;
use coffret_model::{EntryLocation, EntryMetadata, EntryPath};

use crate::fetch::fetch_error::{FetchError, FetchResult};

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
    /// Journal record, or a diagnostic event.
    pub(super) place: LocalPlace,
}

impl Target {
    /// The Library position this target stands at.
    pub(super) fn path(&self) -> &EntryPath {
        self.location.path()
    }

    /// What the Container's own entry table records about this Entry, found
    /// by its entry number (spec: FM-9).
    ///
    /// By number and never by path: a rename changes the path the catalog
    /// holds and leaves the one the table recorded, so a path lookup would
    /// miss a renamed Entry or, after two Entries swapped names, find the
    /// other one (spec: FM-15). A table too short to hold the number is
    /// [`FetchError::EntryMissing`], with no other lookup tried.
    pub(super) fn recorded_in<'o>(
        &self,
        outline: &'o ContainerOutline,
    ) -> FetchResult<&'o EntryMetadata> {
        outline
            .entry_numbered(self.location.entry_number)
            .ok_or_else(|| FetchError::EntryMissing {
                container_id: self.location.container_id,
                entry_number: self.location.entry_number,
                path: self.path().clone(),
            })
    }
}
