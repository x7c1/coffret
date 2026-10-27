use coffret_model::LibraryId;

/// A whole Library on this device, as a person choosing one is shown it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryOnDevice {
    /// The Library this is (spec: FM-18).
    pub library_id: LibraryId,
    /// What it is listed under: the copy of its name its settings keep, or
    /// the directory an earlier build named after it. `None` for a Library
    /// whose catalog names nothing yet, which is shown as having no name
    /// rather than under its ID.
    pub name: Option<String>,
    /// Whether more than one directory on this device holds it: a device holds
    /// a Library once, so it is listed once, and opening it is refused, with
    /// both directories named, until one of them is gone.
    pub needs_attention: bool,
}
