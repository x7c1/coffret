use std::error;
use std::fmt;

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong when building a domain value from raw input.
#[derive(Debug, Clone)]
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
    /// A Master Key epoch number falls outside the range epochs are numbered in.
    ///
    /// Numbering starts at 1, so 0 names no epoch, and the last representable
    /// epoch has no successor to rotate into.
    EpochOutOfRange,
    /// The last representable generation has no successor to write next.
    GenerationOutOfRange,
    /// A replica index does not name a replica the count declares.
    InvalidReplicaPosition {
        /// The 0-based index supplied.
        index: u16,
        /// The replica count supplied.
        count: u16,
    },
    /// A control object's name is not one of the forms FM-12 defines.
    MalformedObjectName {
        /// The name as it was presented.
        name: String,
    },
    /// A Keyring `set_digest` is not the non-empty lowercase hex token FM-12
    /// spells it as.
    ///
    /// Uppercase is rejected for the same reason a hex identifier's is: two
    /// spellings of one digest would name one replica set twice, while a commit
    /// selects a set by its exact tuple (KL-3, CP-10).
    InvalidSetDigest {
        /// The digest as it was presented.
        digest: String,
    },
    /// A Storage key prefix a Library's app folder was to be placed under is
    /// neither empty nor terminated by `/` (FM-18).
    ///
    /// Appending to such a base would run it into the folder's own name and
    /// place the Library where the caller did not ask for it, so it is refused
    /// rather than corrected.
    MalformedPrefixBase {
        /// The base as it was presented.
        base: String,
    },
    /// A Keyring replica count declares no replica.
    ///
    /// A set of zero replicas can never be complete, so no commit can ever
    /// select it (KL-2, KL-3).
    InvalidReplicaCount,
    /// A path the Library already holds is not the NFC spelling every Entry
    /// Path is in (EP-1).
    ///
    /// Text from outside the Library is composed on the way in, so a stored
    /// path that is not NFC was never written by anything holding to that rule:
    /// it is malformed data. Why it is refused rather than composed is on
    /// [`EntryPath`](crate::EntryPath).
    ///
    /// The offending path travels in the value, which is where a caller
    /// reporting the record it could not read has to get it from. It is Library
    /// content all the same, so it belongs in no log field.
    UnnormalizedEntryPath {
        /// The path as it was stored.
        path: String,
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
            Self::EpochOutOfRange => f.write_str("Master Key epochs are numbered from 1 upward"),
            Self::GenerationOutOfRange => {
                f.write_str("the last representable generation has no successor")
            }
            Self::InvalidReplicaPosition { index, count } => {
                write!(f, "replica {index} is not one of {count} replicas")
            }
            Self::MalformedObjectName { name } => {
                write!(f, "{name:?} is not a control-object name")
            }
            Self::InvalidSetDigest { digest } => {
                write!(f, "{digest:?} is not a lowercase hex Keyring digest")
            }
            Self::MalformedPrefixBase { base } => {
                write!(f, "the prefix {base:?} does not end in a \"/\"")
            }
            Self::InvalidReplicaCount => {
                f.write_str("a Keyring replica set declares at least one replica")
            }
            Self::UnnormalizedEntryPath { path } => {
                write!(f, "the stored path {path:?} is not normalized to NFC")
            }
        }
    }
}

impl error::Error for Error {}
