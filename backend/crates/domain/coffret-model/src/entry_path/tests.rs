//! What each side of the Library boundary reads, refuses, and derives.

use super::*;
use crate::error::Error;

/// `café.jpg` with the accent as one code point, which is its NFC spelling.
const COMPOSED: &str = "caf\u{e9}.jpg";

/// The same name with the accent as `e` and a combining acute, which is what
/// some filesystems hand a scan back.
const DECOMPOSED: &str = "cafe\u{301}.jpg";

/// The path `text` parses to, or a panic naming what it was refused for.
fn parsed(text: &str) -> EntryPath {
    EntryPath::parse(text)
        .unwrap_or_else(|error| panic!("a case holds a literal Entry Path: {error}"))
}

// EP-1: a name from outside the Library arrives in whichever spelling its
// filesystem keeps, and leaves this constructor in the Library's.
#[test]
fn text_from_outside_is_composed() {
    assert_eq!(parsed(DECOMPOSED).as_str(), COMPOSED);
}

// The ordinary case: almost every name a scan reads is already NFC, and
// composing it again must leave it exactly where it was.
#[test]
fn composing_text_already_in_nfc_changes_nothing() {
    assert_eq!(parsed(COMPOSED).as_str(), COMPOSED);
    assert_eq!(parsed("albums/2026/a.jpg").as_str(), "albums/2026/a.jpg");
}

// The same constructor reached the way a literal reads best, so that a
// caller with a path in hand has no reason to reach for anything looser.
#[test]
fn a_literal_parses_through_from_str() {
    let path: EntryPath = "albums/a.jpg".parse().expect("a literal Entry Path");
    assert_eq!(path.as_str(), "albums/a.jpg");
}

// EP-2: what an Entry Path may not be, and which part of the shape each of
// them fails. `..` is the one worth naming twice — a path that climbed out
// of a mapped folder would be a path the Library never held, and no value
// of this type can be one.
#[test]
fn every_shape_ep_2_excludes_cannot_be_parsed() {
    for (text, expected) in [
        ("", PathDefect::Empty),
        ("albums/spring\0.jpg", PathDefect::Nul),
        ("/albums/spring.jpg", PathDefect::LeadingSeparator),
        ("albums/spring.jpg/", PathDefect::TrailingSeparator),
        ("albums//spring.jpg", PathDefect::EmptyComponent),
        ("albums/../../etc/passwd", PathDefect::RelativeComponent),
        ("..", PathDefect::RelativeComponent),
        ("albums/./spring.jpg", PathDefect::RelativeComponent),
    ] {
        let result = EntryPath::parse(text);
        assert!(
            matches!(
                &result,
                Err(Error::MalformedEntryPath { path, defect })
                    if path == text && *defect == expected
            ),
            "expected {text:?} to be refused for {expected:?}, got {result:?}"
        );
    }
}

#[test]
fn an_ordinary_path_is_parsed() {
    for text in ["notes.txt", "albums/2026/08/spring.jpg", "books-annex"] {
        assert_eq!(parsed(text).as_str(), text);
    }
}

// A component that merely begins with a dot is a name, not a relative
// reference: `.hidden` and `...three` are files somebody has.
#[test]
fn a_name_that_starts_with_a_dot_is_a_name() {
    assert_eq!(parsed("albums/.hidden").as_str(), "albums/.hidden");
    assert_eq!(parsed("albums/...three").as_str(), "albums/...three");
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

// EP-2: a stored path outside the shape is malformed data in exactly the
// way a decomposed one is — nothing holding to the rule could have written
// it — and it is refused rather than trimmed into something plausible.
#[test]
fn a_stored_path_with_a_shape_ep_2_excludes_is_refused() {
    for (text, expected) in [
        ("", PathDefect::Empty),
        ("albums/spring\0.jpg", PathDefect::Nul),
        ("/albums/spring.jpg", PathDefect::LeadingSeparator),
        ("albums/spring.jpg/", PathDefect::TrailingSeparator),
        ("albums//spring.jpg", PathDefect::EmptyComponent),
        ("albums/../../etc/passwd", PathDefect::RelativeComponent),
        ("..", PathDefect::RelativeComponent),
        ("albums/./spring.jpg", PathDefect::RelativeComponent),
    ] {
        let result = EntryPath::stored(text);
        assert!(
            matches!(
                &result,
                Err(Error::MalformedEntryPath { path, defect })
                    if path == text && *defect == expected
            ),
            "expected the stored path {text:?} to be refused for {expected:?}, \
             got {result:?}"
        );
    }

    // Neither rule holds of this one, and the normal form is what `stored`
    // asks about first, so that is the answer it gives.
    let both = format!("../{DECOMPOSED}");
    let result = EntryPath::stored(both.clone());
    assert!(
        matches!(&result, Err(Error::UnnormalizedEntryPath { path }) if *path == both),
        "expected the normal form to be the first thing asked about, got {result:?}"
    );
}

// EP-2: two shaped paths joined by the one separator are a shaped path, so
// the join answers what parsing the joined text would have — which is what
// lets it have no refusal to report.
#[test]
fn a_path_below_another_is_a_path() {
    for (top, relative) in [
        ("albums", "spring.jpg"),
        ("albums/2026", "08/spring.jpg"),
        ("books", "atlas"),
    ] {
        let joined = parsed(top).below(&parsed(relative));
        assert_eq!(joined, parsed(&format!("{top}/{relative}")));
    }
}

// EP-2, EP-9: `/` is the only logical separator, so a prefix covers the
// Entry at exactly that path and everything under `prefix/` — never a
// sibling whose name merely starts with the same letters.
#[test]
fn a_prefix_covers_itself_and_its_subtree_only() {
    let prefix = parsed("books");
    assert!(parsed("books").is_under(&prefix));
    assert!(parsed("books/page-1.png").is_under(&prefix));
    assert!(!parsed("books-annex/page-1.png").is_under(&prefix));
    assert!(!parsed("albums/spring.jpg").is_under(&prefix));
}

// EP-2: the dual of the parent, and the two of them are the whole path.
#[test]
fn the_name_is_the_last_component() {
    assert_eq!(parsed("albums/2026/spring.jpg").name(), "spring.jpg");
    assert_eq!(parsed("notes.txt").name(), "notes.txt");
}

#[test]
fn the_top_level_is_the_first_component() {
    assert_eq!(parsed("albums/2026/spring.jpg").top_level(), "albums");
    assert_eq!(parsed("notes.txt").top_level(), "notes.txt");
}

// EP-2: the cut is at the last separator, and a path with none of them
// stands in the Library root, which is no folder at all.
#[test]
fn the_parent_is_everything_before_the_last_separator() {
    assert_eq!(
        parsed("albums/2026/spring.jpg").parent(),
        Some(parsed("albums/2026"))
    );
    assert_eq!(parsed("albums/cover.png").parent(), Some(parsed("albums")));
    assert_eq!(parsed("notes.txt").parent(), None);
}
