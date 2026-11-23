use coffret_model::{EntryPath, Mtime};

/// One folder inside another, and what the Entries at every depth below it
/// come to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderSummary {
    /// Where in the Library the folder stands.
    pub path: EntryPath,
    /// How many current Entries stand at every depth below it. Never zero: a
    /// folder of the Library always has an Entry under it (spec: EP-2).
    pub files_total: u64,
    /// How many of those this device has, by its present materialization
    /// records (spec: EP-10).
    pub files_present: u64,
    /// The plaintext bytes of those `files_total` Entries together.
    pub size: u64,
    /// The latest modification time among those Entries (spec: FM-9).
    pub mtime: Mtime,
}
