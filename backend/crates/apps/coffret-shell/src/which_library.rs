//! How every binary is told which Library on this device it is about.
//!
//! By `--library <name>` or by `--library-id <id>`, and never one flag for
//! both: a Library's name is anything a person chose, including a string that
//! spells another Library's ID, so a flag taking either could not tell the two
//! apart. Each binary declares the two flags in its own parser, exactly one of
//! them required; what is shared is what they mean.

use coffret_device::{LibraryId, LibraryRef};

/// A Library ID as `--library-id` takes it: its 16 lowercase hex characters.
///
/// For a parser's `value_parser`, so that an ID that is not one is refused as
/// a usage error naming the flag.
pub fn library_id(typed: &str) -> Result<LibraryId, String> {
    LibraryId::from_hex(typed).map_err(|refused| {
        format!(
            "a Library ID is 16 lowercase hex characters, as `coffret init` prints it: {refused}"
        )
    })
}

/// The Library the two flags name, of which a parser has taken exactly one.
///
/// # Panics
///
/// Where both or neither is given, which a parser declaring the two as one
/// required group of which only one may be given never hands over.
pub fn library(name: Option<String>, id: Option<LibraryId>) -> LibraryRef {
    match (name, id) {
        (Some(name), None) => LibraryRef::named(name),
        (None, Some(id)) => LibraryRef::Id(id),
        _ => unreachable!("a parser takes exactly one of --library and --library-id"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_its_hex_and_nothing_else() {
        assert_eq!(
            library_id("0123456789abcdef").unwrap().to_hex(),
            "0123456789abcdef"
        );
        for typed in ["books", "0123456789ABCDEF", "0123456789abcde", ""] {
            assert!(library_id(typed).is_err(), "{typed:?} is no Library ID");
        }
    }

    #[test]
    fn a_name_and_an_id_stay_what_they_were_given_as() {
        assert_eq!(
            library(Some("0123456789abcdef".to_owned()), None),
            LibraryRef::named("0123456789abcdef")
        );
        let id = library_id("0123456789abcdef").unwrap();
        assert_eq!(library(None, Some(id)), LibraryRef::Id(id));
    }
}
