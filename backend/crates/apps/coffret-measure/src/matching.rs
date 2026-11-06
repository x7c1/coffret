//! Matching a typed name against Entry Paths, one path segment at a time.
//!
//! An Entry Path is one string whose only separator is `/`, so a name a person
//! types is looked for inside each of its segments — a folder's name or the
//! file's own — and never across a `/`: "a/b" is not in any segment, so it
//! matches nothing. Both sides are compared normalized: Unicode NFKC, which
//! makes full-width and half-width forms one, then lowercased, which makes
//! case not matter.
//!
//! Lowercasing stands in for full case folding here. They differ for a few
//! letters (`ß` folds to `ss` and lowercases to itself), none of which the
//! measurement turns on: what is measured is the cost of normalizing every
//! segment of every Entry, and that is the same either way.

use unicode_normalization::UnicodeNormalization;

/// `text` as it is compared: NFKC, then lowercased.
pub fn normalize(text: &str) -> String {
    text.nfkc().collect::<String>().to_lowercase()
}

/// A typed name, normalized once, to match Entry Paths against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    normalized: String,
}

impl Query {
    pub fn new(typed: &str) -> Self {
        Self {
            normalized: normalize(typed),
        }
    }

    /// Whether the query stands inside one of `path`'s segments.
    ///
    /// Each segment is normalized as it is reached, every time: this is the
    /// full scan being measured, with nothing kept from one query to the next.
    pub fn matches(&self, path: &str) -> bool {
        path.split('/')
            .any(|segment| normalize(segment).contains(&self.normalized))
    }

    /// How many of `paths` the query matches.
    pub fn count<'a>(&self, paths: impl IntoIterator<Item = &'a str>) -> usize {
        paths.into_iter().filter(|path| self.matches(path)).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_a_substring_of_any_segment() {
        let query = Query::new("lantern");
        assert!(query.matches("Comics/Star Lantern/Vol. 01/001.jpg"));
        assert!(query.matches("Comics/Series/Vol. 01/lanterns.jpg"));
        assert!(Query::new("Vol").matches("Comics/Star Lantern/Vol. 01/001.jpg"));
        assert!(!query.matches("Comics/Star Lane/Vol. 01/001.jpg"));
    }

    #[test]
    fn never_matches_across_a_separator() {
        let path = "Comics/Star Lantern/Vol. 01/001.jpg";
        assert!(!Query::new("lantern/vol").matches(path));
        assert!(!Query::new("comics/").matches(path));
        // The two segments run together would hold it; no one segment does.
        assert!(!Query::new("slantern").matches("Comics/Bus/lantern.jpg"));
        assert!(!Query::new("buslantern").matches("Comics/Bus/lantern.jpg"));
    }

    #[test]
    fn full_width_and_half_width_are_one() {
        let full_width = "Comics/ＳＴＡＲ ＬＡＮＴＥＲＮ/Vol. 01/001.jpg";
        assert!(Query::new("star lantern").matches(full_width));
        assert!(Query::new("ＳＴＡＲ").matches("Comics/Star Lantern/Vol. 01/001.jpg"));
        // Half-width katakana is the full-width katakana after NFKC.
        assert!(Query::new("ｺｺﾛ").matches("Novels/中村/ココロ/001.jpg"));
        assert!(Query::new("ココロ").matches("Novels/中村/ｺｺﾛ/001.jpg"));
        // Full-width digits are digits.
        assert!(Query::new("2019").matches("Magazines/月刊/２０１９/01.jpg"));
    }

    #[test]
    fn case_does_not_matter() {
        let path = "Novels/Margaret Ashford/The Quiet Harbor/001.jpg";
        assert!(Query::new("quiet harbor").matches(path));
        assert!(Query::new("QUIET HARBOR").matches(path));
        assert!(Query::new("ｑｕｉｅｔ").matches(path));
    }

    #[test]
    fn matches_japanese_names_as_they_are() {
        let path = "Novels/佐藤春樹/月の海辺/表紙.jpg";
        assert!(Query::new("海辺").matches(path));
        assert!(Query::new("表紙").matches(path));
        assert!(!Query::new("海辺/表紙").matches(path));
        assert!(!Query::new("灯台").matches(path));
    }

    #[test]
    fn counts_the_paths_it_matches() {
        let paths = [
            "Comics/Star Lantern/Vol. 01/001.jpg",
            "Comics/Star Lantern/Vol. 02/001.jpg",
            "Novels/Lanternkeeper/001.jpg",
            "Novels/Other/001.jpg",
        ];
        assert_eq!(Query::new("lantern").count(paths), 3);
        assert_eq!(Query::new("vol").count(paths), 2);
        assert_eq!(Query::new("nothing like it").count(paths), 0);
    }
}
