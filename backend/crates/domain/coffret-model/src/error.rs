use std::error;
use std::fmt;

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong when building a domain value from raw input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A hex string did not have the number of characters the target type needs.
    InvalidHexLength {
        /// Characters the target type requires.
        expected: usize,
        /// Characters actually supplied.
        actual: usize,
    },
    /// A hex string held a character outside `0-9a-f`.
    ///
    /// Uppercase is rejected as well: identifiers are canonically lowercase, so
    /// accepting both cases would let two spellings name the same value.
    InvalidHexDigit {
        /// The offending character.
        found: char,
    },
    /// A byte slice did not have the length the target type needs.
    InvalidByteLength {
        /// Bytes the target type requires.
        expected: usize,
        /// Bytes actually supplied.
        actual: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHexLength { expected, actual } => {
                write!(f, "expected {expected} hex characters, found {actual}")
            }
            Self::InvalidHexDigit { found } => {
                write!(f, "expected a lowercase hex character, found {found:?}")
            }
            Self::InvalidByteLength { expected, actual } => {
                write!(f, "expected {expected} bytes, found {actual}")
            }
        }
    }
}

impl error::Error for Error {}
