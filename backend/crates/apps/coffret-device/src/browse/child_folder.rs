use coffret_model::EntryPath;

/// One folder inside another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildFolder {
    /// Its last path component, which is what it is called.
    pub name: String,
    /// Where in the Library it stands.
    pub path: EntryPath,
}
