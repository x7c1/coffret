use std::fmt;

/// What one add is called while it is under way, so that a page can ask for
/// it to be stopped before the request carrying it has been answered
/// (spec: LA-35).
///
/// Chosen by the page that makes the add and named in its request, since a
/// stop pressed while the files are still being sent has nothing else to name
/// it by; drawn here where the request names none. It names no file and no
/// folder: it is a token of the page's, which a page is free to make up, and
/// nothing a person wrote.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AddId(String);

/// The longest a page's name for an add may be.
const LONGEST: usize = 64;

impl AddId {
    /// The name a page gave an add, where it is one: one to sixty-four ASCII
    /// letters, digits, `-` or `_` — enough for a UUID, and nothing that needs
    /// escaping anywhere it is written.
    pub fn named(asked: &str) -> Option<Self> {
        let fits = !asked.is_empty()
            && asked.len() <= LONGEST
            && asked
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
        fits.then(|| Self(asked.to_owned()))
    }

    /// A name drawn by this server, for an add whose request named none: the
    /// `n`-th such add of this process.
    pub(crate) fn drawn(n: u64) -> Self {
        Self(format!("server-{n}"))
    }

    /// The name as a page sends it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AddId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::AddId;

    // LA-35: a page names an add by one to sixty-four ASCII letters, digits,
    // `-` or `_`, and one it named nothing is given a name by the server.
    #[test]
    fn a_page_names_an_add_with_a_short_token() {
        assert!(AddId::named("3f2c9a7e-1b2d-4c5e-8f90-123456789abc").is_some());
        assert!(AddId::named("").is_none());
        assert!(AddId::named("a/b").is_none());
        assert!(AddId::named(&"x".repeat(65)).is_none());
        assert!(AddId::named(&"x".repeat(64)).is_some());
        assert_eq!(AddId::drawn(3).as_str(), "server-3");
    }
}
