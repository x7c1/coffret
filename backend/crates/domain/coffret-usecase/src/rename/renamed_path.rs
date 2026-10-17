use coffret_model::EntryPath;

/// One Entry a rename gave a new current Entry Path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamedPath {
    /// The path the Entry stood at.
    pub from: EntryPath,
    /// The path it stands at now.
    pub to: EntryPath,
}
