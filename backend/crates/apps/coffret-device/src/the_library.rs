use std::fmt;

use crate::library_ref::LibraryRef;

impl LibraryRef {
    /// The Library as a sentence names it: by its name, quoted, or — for a
    /// Library this device knows no name for — said to have none and named by
    /// its ID as its ID, never with its ID in the name's place (spec: FM-15).
    ///
    /// The one place that wording is built, so that every refusal, report and
    /// answer naming a Library says it alike.
    pub fn the_library(&self) -> TheLibrary<'_> {
        TheLibrary(self)
    }
}

/// [`LibraryRef::the_library`]: a Library as a sentence names it.
#[derive(Debug, Clone, Copy)]
pub struct TheLibrary<'a>(&'a LibraryRef);

impl fmt::Display for TheLibrary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            LibraryRef::Named(name) => write!(f, "the Library {name:?}"),
            LibraryRef::Id(id) => write!(f, "the Library with ID {id} (it has no name yet)"),
        }
    }
}
