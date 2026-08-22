use coffret_usecase::{Error, Result};

/// Where the two states of an object live in the bucket's key space.
///
/// S3 has no notion of a trash, so this gateway makes one out of keys: live
/// objects sit directly under the configured prefix and trashed ones under a
/// reserved `trash/` segment of it. Because live names are flat, a listing that
/// asks S3 to collapse everything past a `/` sees the trash as a single common
/// prefix and never as an object, so trashed objects stay out of
/// [`list`](coffret_usecase::ObjectStore::list) without the gateway having to
/// drop them from every page it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyLayout {
    prefix: String,
}

/// The segment of the key space trashed objects are moved into.
const TRASH_SEGMENT: &str = "trash/";

/// The separator S3 listings collapse on, and therefore the one character an
/// object name may not contain.
pub const DELIMITER: &str = "/";

impl KeyLayout {
    /// Takes the prefix every key of this Library starts with.
    ///
    /// An empty prefix puts the Library at the root of the bucket; a non-empty
    /// one is normalized to end in a separator, so `libraries/alpha` and
    /// `libraries/alpha/` describe the same key space rather than two.
    pub fn new(prefix: &str) -> Self {
        let prefix = match prefix {
            "" => String::new(),
            _ if prefix.ends_with(DELIMITER) => prefix.to_owned(),
            _ => format!("{prefix}{DELIMITER}"),
        };
        Self { prefix }
    }

    /// The prefix a listing of the live objects asks for.
    pub fn live_prefix(&self) -> &str {
        &self.prefix
    }

    /// The key a live object is stored under.
    pub fn live_key(&self, name: &str) -> String {
        format!("{}{name}", self.prefix)
    }

    /// The key a trashed object is stored under.
    pub fn trashed_key(&self, name: &str) -> String {
        format!("{}{TRASH_SEGMENT}{name}", self.prefix)
    }

    /// The name a live key stores, or `None` if the key is not one of ours.
    pub fn name_of<'a>(&self, key: &'a str) -> Option<&'a str> {
        key.strip_prefix(&self.prefix)
            .filter(|name| !name.is_empty() && !name.contains(DELIMITER))
    }

    /// Checks a name is one this layout can store and hand back unchanged.
    ///
    /// Two things would go wrong quietly otherwise. A name carrying a separator
    /// lands in a nested key that the live listing collapses away, so the object
    /// would be stored and then be invisible. A name carrying anything outside
    /// the URL-unreserved set has to be escaped in the `x-amz-copy-source` a
    /// trash move is built from, and one escaping mistake there moves an object
    /// to a key nobody will look under.
    ///
    /// Every name coffret stores — a Container's 32 hex characters followed by
    /// `.cfrt` (spec: FM-3), a control object's `head-`/`idx-`/`key-` name
    /// (spec: FM-12) — is already within that set, so this rejects mistakes
    /// rather than legitimate names.
    pub fn validate(&self, name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(Error::Unsupported {
                detail: "an object name cannot be empty".to_owned(),
            });
        }
        let unreserved =
            |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~');
        if !name.bytes().all(unreserved) {
            return Err(Error::Unsupported {
                detail: format!("an object name must be URL-unreserved ASCII: {name:?}"),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prefix_without_a_separator_gains_one() {
        let layout = KeyLayout::new("libraries/alpha");
        assert_eq!(
            layout.live_key("head-1.cfrt"),
            "libraries/alpha/head-1.cfrt"
        );
        assert_eq!(
            layout.trashed_key("head-1.cfrt"),
            "libraries/alpha/trash/head-1.cfrt"
        );
    }

    #[test]
    fn an_empty_prefix_puts_the_library_at_the_bucket_root() {
        let layout = KeyLayout::new("");
        assert_eq!(layout.live_key("head-1.cfrt"), "head-1.cfrt");
        assert_eq!(layout.trashed_key("head-1.cfrt"), "trash/head-1.cfrt");
    }

    #[test]
    fn only_live_keys_carry_a_name() {
        let layout = KeyLayout::new("alpha/");
        assert_eq!(layout.name_of("alpha/head-1.cfrt"), Some("head-1.cfrt"));
        assert_eq!(layout.name_of("alpha/trash/head-1.cfrt"), None);
        assert_eq!(layout.name_of("beta/head-1.cfrt"), None);
        assert_eq!(layout.name_of("alpha/"), None);
    }

    #[test]
    fn a_name_that_would_vanish_from_the_listing_is_refused() {
        let layout = KeyLayout::new("alpha/");
        assert!(matches!(
            layout.validate("nested/head-1.cfrt"),
            Err(Error::Unsupported { .. })
        ));
        assert!(matches!(
            layout.validate(""),
            Err(Error::Unsupported { .. })
        ));
    }

    #[test]
    fn a_name_needing_escaping_is_refused() {
        let layout = KeyLayout::new("alpha/");
        assert!(matches!(
            layout.validate("head 1.cfrt"),
            Err(Error::Unsupported { .. })
        ));
        assert!(matches!(
            layout.validate("head-1.cfrt?x=1"),
            Err(Error::Unsupported { .. })
        ));
    }

    #[test]
    fn the_names_coffret_stores_are_accepted() {
        let layout = KeyLayout::new("alpha/");
        assert!(layout.validate("head-1.cfrt").is_ok());
        assert!(layout.validate("key-3-a1b2-r0-of-2.cfrt").is_ok());
        assert!(layout
            .validate("0123456789abcdef0123456789abcdef.cfrt")
            .is_ok());
    }
}
