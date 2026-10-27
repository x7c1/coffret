//! How a caller says which Library on this device it means.

use std::fmt;

use coffret_model::LibraryId;

/// Which Library on this device a call is about: by its name, or by its ID.
///
/// Two variants and never one string read either way. A Library's name is
/// anything a person chose (spec: FM-15), including a string that spells
/// another Library's ID, so a single argument taking either could not tell the
/// two apart; a caller says which one it was given.
///
/// A name is matched against the copy of each Library's name this device keeps
/// beside it, and more than one Library on a device may carry one name — two
/// Libraries a person created under it, or one of theirs and one they joined.
/// Such a name is refused rather than chosen between, and the ID is what tells
/// them apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryRef {
    /// The Library carrying this name.
    Named(String),
    /// The Library with this ID (spec: FM-18).
    Id(LibraryId),
}

impl LibraryRef {
    /// The Library carrying `name`.
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(name.into())
    }
}

impl fmt::Display for LibraryRef {
    /// The name as it was given, or the ID in the hex it is spelled in.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(name) => f.write_str(name),
            Self::Id(id) => f.write_str(&id.to_hex()),
        }
    }
}

/// The Library name `name` spells, or the refusal saying what is wrong with it
/// (spec: FM-15).
pub(crate) fn library_name(name: &str) -> crate::error::Result<coffret_model::LibraryName> {
    coffret_model::LibraryName::parse(name).map_err(|refused| match refused {
        coffret_model::Error::InvalidLibraryName { name, defect } => {
            crate::error::Error::InvalidLibraryName { name, defect }
        }
        other => unreachable!("a name is refused only as a name, not as {other:?}"),
    })
}
