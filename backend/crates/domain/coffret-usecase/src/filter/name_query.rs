use unicode_normalization::UnicodeNormalization;

/// A typed name, to match Entry Paths against one segment at a time
/// (spec: LA-40).
///
/// An Entry Path is one string whose only separator is `/` (spec: EP-2), so a
/// name a person types is looked for inside each of its segments — a folder's
/// name or the file's own — and never across a `/`: "a/b" is in no segment,
/// so it matches nothing. Both sides are compared normalized: Unicode NFKC,
/// which makes full-width and half-width forms one, then lowercased, which
/// makes case not matter.
///
/// Lowercasing stands in for full case folding. The two differ for a few
/// letters (`ß` folds to `ss` and lowercases to itself), which a substring
/// match over names a person typed does not turn on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameQuery {
    normalized: String,
}

impl NameQuery {
    /// The query a person typed, or `None` where it is empty once normalized:
    /// an empty name is in every segment, so it would select everything and
    /// filter nothing.
    pub fn new(typed: &str) -> Option<Self> {
        let normalized = normalize(typed);
        (!normalized.is_empty()).then_some(Self { normalized })
    }

    /// Whether the query stands inside one segment, a name with no `/` in it.
    fn matches_segment(&self, segment: &str) -> bool {
        normalize(segment).contains(&self.normalized)
    }

    /// Whether the query stands inside any one of `path`'s segments.
    pub fn matches(&self, path: &str) -> bool {
        self.shallowest_match(path).is_some()
    }

    /// How much of `path` names the shallowest folder or file the query
    /// matches: the length, in bytes, of the prefix ending with the first
    /// segment the query stands inside, and `None` where it stands inside
    /// none.
    ///
    /// The prefix is the whole path where only the last segment matches, which
    /// is the file itself.
    pub fn shallowest_match(&self, path: &str) -> Option<usize> {
        let mut start = 0;
        for segment in path.split('/') {
            let end = start + segment.len();
            if self.matches_segment(segment) {
                return Some(end);
            }
            start = end + 1;
        }
        None
    }
}

/// `text` as it is compared: NFKC, then lowercased.
fn normalize(text: &str) -> String {
    text.nfkc().collect::<String>().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::NameQuery;

    fn query(typed: &str) -> NameQuery {
        NameQuery::new(typed).expect("a name that is not empty")
    }

    // LA-40: a name is a substring of any one segment of a path.
    #[test]
    fn matches_a_substring_of_any_segment() {
        let lantern = query("lantern");
        assert!(lantern.matches("Comics/Star Lantern/Vol. 01/001.jpg"));
        assert!(lantern.matches("Comics/Series/Vol. 01/lanterns.jpg"));
        assert!(query("Vol").matches("Comics/Star Lantern/Vol. 01/001.jpg"));
        assert!(!lantern.matches("Comics/Star Lane/Vol. 01/001.jpg"));
    }

    // LA-40: never across a `/`.
    #[test]
    fn never_matches_across_a_separator() {
        let path = "Comics/Star Lantern/Vol. 01/001.jpg";
        assert!(!query("lantern/vol").matches(path));
        assert!(!query("comics/").matches(path));
        // The two segments run together would hold it; no one segment does.
        assert!(!query("buslantern").matches("Comics/Bus/lantern.jpg"));
        assert!(!query("s/l").matches("Comics/Bus/lantern.jpg"));
    }

    // LA-40: NFKC on both sides, so full-width and half-width forms are one.
    #[test]
    fn full_width_and_half_width_are_one() {
        let full_width = "Comics/ＳＴＡＲ ＬＡＮＴＥＲＮ/Vol. 01/001.jpg";
        assert!(query("star lantern").matches(full_width));
        assert!(query("ＳＴＡＲ").matches("Comics/Star Lantern/Vol. 01/001.jpg"));
        // Half-width katakana is the full-width katakana after NFKC.
        assert!(query("ｺｺﾛ").matches("Novels/中村/ココロ/001.jpg"));
        assert!(query("ココロ").matches("Novels/中村/ｺｺﾛ/001.jpg"));
        // Full-width digits are digits.
        assert!(query("2019").matches("Magazines/月刊/２０１９/01.jpg"));
    }

    // LA-40: case does not matter.
    #[test]
    fn case_does_not_matter() {
        let path = "Novels/Margaret Ashford/The Quiet Harbor/001.jpg";
        assert!(query("quiet harbor").matches(path));
        assert!(query("QUIET HARBOR").matches(path));
        assert!(query("ｑｕｉｅｔ").matches(path));
    }

    #[test]
    fn matches_japanese_names_as_they_are() {
        let path = "Novels/架空の作家/月の海辺/表紙.jpg";
        assert!(query("海辺").matches(path));
        assert!(query("表紙").matches(path));
        assert!(!query("海辺/表紙").matches(path));
        assert!(!query("灯台").matches(path));
    }

    #[test]
    fn the_shallowest_match_ends_with_the_first_segment_matched() {
        let path = "Comics/Star Lantern/Lantern 01/001.jpg";
        assert_eq!(
            query("lantern")
                .shallowest_match(path)
                .map(|end| &path[..end]),
            Some("Comics/Star Lantern"),
        );
        assert_eq!(
            query("001.jpg")
                .shallowest_match(path)
                .map(|end| &path[..end]),
            Some(path),
            "only the file's own name matches, so the prefix is the whole path",
        );
        assert_eq!(query("comics").shallowest_match(path), Some(6));
        assert_eq!(query("harbor").shallowest_match(path), None);
    }

    #[test]
    fn an_empty_name_is_no_query() {
        assert_eq!(NameQuery::new(""), None);
    }
}
