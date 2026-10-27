use coffret_model::LibraryId;

/// A whole Library on this device, as a person choosing one is shown it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryOnDevice {
    /// The Library this is (spec: FM-18).
    pub library_id: LibraryId,
    /// What it is listed under: the copy of its name its settings keep, or
    /// the directory an earlier build named after it. `None` for a Library
    /// whose catalog names nothing yet.
    pub name: Option<String>,
}
