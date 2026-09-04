use coffret_model::EntryPath;

/// The only logical separator an Entry Path has (spec: EP-2).
const SEPARATOR: u8 = b'/';

/// The byte after the separator.
///
/// Every path under `prefix/` begins with the separator's byte, and none
/// begins with the one after it, so the two make a half-open range that the
/// primary key index answers directly.
const AFTER_SEPARATOR: u8 = SEPARATOR + 1;

/// The half-open range of Entry Paths lying beneath `prefix`.
///
/// The Entry at exactly `prefix` is not in it — a prefix covers that path too,
/// and a query names it separately — because the range starts at the separator.
/// What the range does exclude for good is a sibling whose name merely begins
/// with the same letters: `books-annex/page-001.png` sorts between `books` and
/// `books/page-001.png`, and a comparison against the bare prefix would sweep
/// it into the subtree (spec: EP-2, EP-3, EP-9).
pub(crate) fn subtree_range(prefix: &EntryPath) -> (String, String) {
    let mut lower = String::with_capacity(prefix.as_str().len() + 1);
    lower.push_str(prefix.as_str());
    lower.push(char::from(SEPARATOR));

    let mut upper = String::with_capacity(lower.len());
    upper.push_str(prefix.as_str());
    upper.push(char::from(AFTER_SEPARATOR));

    (lower, upper)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Entry Path a literal spells, or a panic naming the one that spells
    /// none (spec: EP-1, EP-2).
    fn path(text: &str) -> EntryPath {
        EntryPath::parse(text)
            .unwrap_or_else(|error| panic!("a case holds a literal Entry Path: {error}"))
    }

    #[test]
    fn the_range_holds_a_subtree_and_not_its_siblings() {
        let (lower, upper) = subtree_range(&path("books"));
        assert_eq!((lower.as_str(), upper.as_str()), ("books/", "books0"));

        let inside = ["books/page-001.png", "books/some-novel/page-042.png"];
        for path in inside {
            assert!(
                lower.as_str() <= path && path < upper.as_str(),
                "{path} lies under the prefix"
            );
        }
        // `-` is 0x2D and `/` is 0x2F, so this sibling sorts between the bare
        // prefix and its subtree — exactly the case the range has to exclude.
        let outside = ["books", "books-annex/page-001.png", "bookshelf.txt"];
        for path in outside {
            assert!(
                !(lower.as_str() <= path && path < upper.as_str()),
                "{path} does not lie under the prefix"
            );
        }
    }
}
