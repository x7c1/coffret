use std::fmt;

use unicode_normalization::{is_nfc, UnicodeNormalization};

use crate::error::{Error, Result};

/// The Library position an Entry occupies.
///
/// Every Entry Path is Unicode normalized to NFC (spec: EP-1), and this type is
/// where that stops being a convention and becomes an invariant: an `EntryPath`
/// exists only in that form, so nothing downstream has to ask whether the path
/// it holds was normalized by whoever built it. Past the normal form the path is
/// opaque and kept verbatim — it composes nothing further and folds nothing
/// together (spec: EP-3).
///
/// There is no way to build one without saying which side of the Library
/// boundary the text came from, because the two sides owe different answers:
///
/// - [`nfc`](Self::nfc) is for text from outside — a name a filesystem handed a
///   scan back, the top-level component a device's mapping is configured with, a
///   prefix a caller narrows a run to. One filesystem spells `é` as a single
///   code point and another as `e` followed by a combining acute, for the very
///   same file, so the spelling becomes the Library's on the way in.
/// - [`stored`](Self::stored) is for text the Library already holds — a decoded
///   meta section, a control payload, a catalog row. EP-1 already holds of
///   those, so one that is not NFC is malformed data and is refused rather than
///   rewritten: composing it would change bytes a digest was taken over and make
///   a stored record decode to something other than what was encoded.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryPath(String);

impl EntryPath {
    /// The Entry Path text from outside the Library stands at, put into the
    /// form every Entry Path is in (spec: EP-1).
    ///
    /// Idempotent: text already in NFC is kept as it stands, which is the
    /// ordinary case and the reason nothing is copied for it.
    pub fn nfc(text: impl Into<String>) -> Self {
        let text = text.into();
        if is_nfc(&text) {
            Self(text)
        } else {
            Self(text.nfc().collect())
        }
    }

    /// The Entry Path a stored path already is, or a refusal where it is not
    /// (spec: EP-1).
    ///
    /// # Errors
    ///
    /// [`Error::UnnormalizedEntryPath`] where `text` is not NFC. What a reader
    /// makes of that refusal is its own layer's business; the one answer no
    /// layer gives is to compose it and carry on.
    pub fn stored(text: impl Into<String>) -> Result<Self> {
        let text = text.into();
        if is_nfc(&text) {
            Ok(Self(text))
        } else {
            Err(Error::UnnormalizedEntryPath { path: text })
        }
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

    /// `café.jpg` with the accent as one code point, which is its NFC spelling.
    const COMPOSED: &str = "caf\u{e9}.jpg";

    /// The same name with the accent as `e` and a combining acute, which is what
    /// some filesystems hand a scan back.
    const DECOMPOSED: &str = "cafe\u{301}.jpg";

    // EP-1: a name from outside the Library arrives in whichever spelling its
    // filesystem keeps, and leaves this constructor in the Library's.
    #[test]
    fn text_from_outside_is_composed() {
        assert_eq!(EntryPath::nfc(DECOMPOSED).as_str(), COMPOSED);
    }

    // The ordinary case: almost every name a scan reads is already NFC, and
    // composing it again must leave it exactly where it was.
    #[test]
    fn composing_text_already_in_nfc_changes_nothing() {
        assert_eq!(EntryPath::nfc(COMPOSED).as_str(), COMPOSED);
        assert_eq!(
            EntryPath::nfc("albums/2026/a.jpg").as_str(),
            "albums/2026/a.jpg"
        );
    }

    #[test]
    fn a_stored_path_in_nfc_is_accepted() {
        let path = EntryPath::stored(COMPOSED).expect("an NFC path is what a reader expects");
        assert_eq!(path.as_str(), COMPOSED);
    }

    // EP-1: a stored path that is not NFC is handed back as a refusal rather
    // than composed, and the refusal carries the offending path, since a caller
    // reporting the row it could not read has nothing else to name it by.
    #[test]
    fn a_stored_path_that_is_not_in_nfc_is_refused() {
        let result = EntryPath::stored(DECOMPOSED);
        assert!(
            matches!(
                &result,
                Err(Error::UnnormalizedEntryPath { path }) if path == DECOMPOSED
            ),
            "expected the decomposed path to be refused as it stands, got {result:?}"
        );
    }

    // EP-2, EP-9: `/` is the only logical separator, so a prefix covers the
    // Entry at exactly that path and everything under `prefix/` — never a
    // sibling whose name merely starts with the same letters.
    #[test]
    fn a_prefix_covers_itself_and_its_subtree_only() {
        let prefix = EntryPath::nfc("books");
        assert!(EntryPath::nfc("books").is_under(&prefix));
        assert!(EntryPath::nfc("books/page-1.png").is_under(&prefix));
        assert!(!EntryPath::nfc("books-annex/page-1.png").is_under(&prefix));
        assert!(!EntryPath::nfc("albums/spring.jpg").is_under(&prefix));
    }

    #[test]
    fn the_top_level_is_the_first_component() {
        assert_eq!(
            EntryPath::nfc("albums/2026/spring.jpg").top_level(),
            "albums"
        );
        assert_eq!(EntryPath::nfc("notes.txt").top_level(), "notes.txt");
    }
}
