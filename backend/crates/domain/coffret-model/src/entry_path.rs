use std::fmt;

/// The Library position an Entry occupies.
///
/// Canonicalization is not implemented yet: this type carries the path as an
/// opaque string and preserves it verbatim, so the rules governing which
/// spellings are legal and how they normalize can be added without changing
/// every holder of a path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryPath(String);

impl EntryPath {
    /// Wraps a path string as-is.
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// The path as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntryPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
