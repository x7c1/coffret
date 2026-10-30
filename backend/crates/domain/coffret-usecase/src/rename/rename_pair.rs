use coffret_model::{EntryPath, Redacted};

/// One `(from, to)` a rename is asked for: the Entry Path renamed — one Entry,
/// or the folder of every Entry under it — and the one it is renamed to.
///
/// A rename takes a set of them and commits them in one Journal record
/// (spec: FM-15), the way a selection is moved at once.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RenamePair {
    /// The Entry Path renamed: an Entry, a folder of Entries, or both where
    /// the Library holds an Entry at a path it also holds Entries under.
    pub from: EntryPath,
    /// The Entry Path it is renamed to.
    pub to: EntryPath,
}

impl RenamePair {
    /// The pair renaming `from` to `to`.
    pub fn new(from: EntryPath, to: EntryPath) -> Self {
        Self { from, to }
    }
}

impl Redacted for RenamePair {
    /// The lengths of the two paths, never the paths (spec: EL-1).
    fn redacted(&self) -> String {
        format!(
            "RenamePair(from_len={}, to_len={})",
            self.from.as_str().len(),
            self.to.as_str().len()
        )
    }
}
