use std::fmt;

use super::JoinedPlace;
use crate::library_ref::LibraryRef;

/// Which Library a [`ClientMismatch`](super::ClientMismatch) is about.
///
/// A Library on this device, or one being created, is named as a command names
/// it; one being joined is known by nothing but the place the join was given,
/// since a join names no Library (spec: FM-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MismatchedLibrary {
    /// A Library opened or created on this device.
    Which(LibraryRef),
    /// The Library a join was given the place of.
    JoinedFrom(JoinedPlace),
}

impl fmt::Display for MismatchedLibrary {
    /// The Library as a sentence names it, or the place it is being joined
    /// from.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Which(library) => write!(f, "{}", library.the_library()),
            Self::JoinedFrom(place) => write!(f, "the Library in {place}"),
        }
    }
}
