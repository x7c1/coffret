use std::fmt;

/// The Library position an Entry occupies.
///
/// This type carries the path as an opaque string and preserves it verbatim: it
/// composes nothing and folds nothing together. Every Entry Path is Unicode
/// normalized to NFC (spec: EP-1), and putting it into that form is the job of
/// whichever boundary constructs one out of text the Library was not already
/// holding: the local scan, where a filesystem's own spelling of a name becomes
/// an Entry Path component, and the reading of a device's mappings, whose
/// prefixes are configuration rather than something the Library handed back.
///
/// Normalizing here instead would reach the paths that come the other way, out
/// of stored objects, and those must stay byte-for-byte what was stored: they
/// are already what EP-1 requires, they are what a digest was taken over, and
/// rewriting one would make a Journal record decode to something other than what
/// was encoded.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryPath(String);

impl EntryPath {
    /// Takes a path string as-is, already in the form its caller owes it
    /// (spec: EP-1).
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// The path as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The first component of the path, which is what a device's mappings are
    /// keyed by (spec: EP-9).
    ///
    /// The whole path where it has only one component, since `/` is the only
    /// logical separator (spec: EP-2).
    pub fn top_level(&self) -> &str {
        self.0.split('/').next().unwrap_or(&self.0)
    }

    /// Whether this path is `prefix` itself or lies beneath it.
    ///
    /// A prefix covers the Entry at exactly that path and everything under
    /// `prefix/`, and nothing else: `books` never covers
    /// `books-annex/page-1.png`, because `/` is the only logical separator
    /// (spec: EP-2, EP-9). It lives here, and not in each caller, because a scan
    /// narrowing to a mapping, a catalog answering `entries_under`, and a fetch
    /// narrowing to a subtree all have to agree on where a subtree ends.
    pub fn is_under(&self, prefix: &Self) -> bool {
        self.0 == prefix.0
            || self
                .0
                .strip_prefix(&prefix.0)
                .is_some_and(|rest| rest.starts_with('/'))
    }
}

impl fmt::Display for EntryPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // EP-2, EP-9: `/` is the only logical separator, so a prefix covers the
    // Entry at exactly that path and everything under `prefix/` — never a
    // sibling whose name merely starts with the same letters.
    #[test]
    fn a_prefix_covers_itself_and_its_subtree_only() {
        let prefix = EntryPath::new("books");
        assert!(EntryPath::new("books").is_under(&prefix));
        assert!(EntryPath::new("books/page-1.png").is_under(&prefix));
        assert!(!EntryPath::new("books-annex/page-1.png").is_under(&prefix));
        assert!(!EntryPath::new("albums/spring.jpg").is_under(&prefix));
    }

    #[test]
    fn the_top_level_is_the_first_component() {
        assert_eq!(
            EntryPath::new("albums/2026/spring.jpg").top_level(),
            "albums"
        );
        assert_eq!(EntryPath::new("notes.txt").top_level(), "notes.txt");
    }
}
