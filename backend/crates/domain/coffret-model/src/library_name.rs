use std::fmt;

use crate::error::{Error, Result};
use crate::library_name_defect::LibraryNameDefect;

/// What a person calls a Library, on every device that holds it.
///
/// The name belongs to the Library rather than to a device: it is chosen once,
/// when the Library is created, and kept in the catalog under the Master Key,
/// so every device that joins reads the same one (spec: FM-15, FM-16). A
/// person knows a Library by one name wherever they open it.
///
/// It names no directory and nothing on Storage — a device keeps a Library in
/// a directory named after its [`LibraryId`](crate::LibraryId), and Storage
/// sees only that ID (spec: FM-18) — so the rules a name is held to are about
/// a person reading it and nothing else: it is not empty, and it holds no
/// control character, which no name should carry and which would leave a
/// person unable to see what they typed. A `/`, a `.` or a `..` are ordinary
/// characters here.
///
/// A name is something a person chose, so it never reaches a diagnostic event
/// (spec: EL-1): nothing that records one may render it, and a refusal that
/// names one renders only its length in a [`Redacted`](crate::Redacted) form.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LibraryName(String);

impl LibraryName {
    /// The name `name` spells, or the refusal saying what is wrong with it.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidLibraryName`], with the [`LibraryNameDefect`] it
    /// failed.
    pub fn parse(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        match defect_in(&name) {
            Some(defect) => Err(Error::InvalidLibraryName { name, defect }),
            None => Ok(Self(name)),
        }
    }

    /// The name, as the person gave it.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The name, consumed.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for LibraryName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What is wrong with `name` as a Library's name, if anything is.
fn defect_in(name: &str) -> Option<LibraryNameDefect> {
    if name.is_empty() {
        return Some(LibraryNameDefect::Empty);
    }
    if name.chars().any(char::is_control) {
        return Some(LibraryNameDefect::Control);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defect(name: &str) -> LibraryNameDefect {
        match LibraryName::parse(name) {
            Err(Error::InvalidLibraryName { defect, .. }) => defect,
            other => panic!("{name:?} must be refused as a Library name, got {other:?}"),
        }
    }

    // A name is held to what a person can read and nothing more: it names no
    // directory, so a separator or a dot is an ordinary character in one.
    #[test]
    fn a_name_is_anything_a_person_can_read() {
        for name in [
            "books",
            "albums/2026",
            "..",
            ".",
            "drafts.partial",
            "写真",
            "a b",
        ] {
            assert_eq!(
                LibraryName::parse(name).expect("a readable name").as_str(),
                name
            );
        }
    }

    #[test]
    fn an_empty_name_or_one_with_a_control_character_is_refused() {
        assert_eq!(defect(""), LibraryNameDefect::Empty);
        assert_eq!(defect("albums\n2026"), LibraryNameDefect::Control);
        assert_eq!(defect("tab\there"), LibraryNameDefect::Control);
    }
}
