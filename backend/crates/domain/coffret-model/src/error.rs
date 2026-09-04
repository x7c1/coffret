use std::error;
use std::fmt;

use crate::{PathDefect, Redacted};

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
    /// A piece of text that had to be an Entry Path is not in the shape EP-2
    /// spells.
    ///
    /// Which part of the shape it fails is carried rather than left to the
    /// reader to work out: text from outside the Library is refused here and
    /// somebody typed it, so "that is not an Entry Path" on its own tells them
    /// nothing to change. A path the Library already holds that is outside the
    /// shape is malformed data in the way an unnormalized one is — nothing
    /// holding to EP-2 could have written it — and the same refusal serves both.
    ///
    /// The offending path travels in the value, for the reason
    /// [`UnnormalizedEntryPath`](Self::UnnormalizedEntryPath)'s does, and it is
    /// Library content all the same: it belongs in no log field.
    MalformedEntryPath {
        /// The path as it was presented.
        path: String,
        /// The part of the shape it fails.
        defect: PathDefect,
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
            Self::MalformedEntryPath { path, defect } => {
                write!(f, "{path:?} is not an Entry Path: {defect}")
            }
        }
    }
}

impl error::Error for Error {}

impl Redacted for Error {
    /// The counts and the positions, and none of the text that was presented.
    ///
    /// Every value this vocabulary refuses arrived from somewhere outside the
    /// process — a control-object name a listing answered with, a digest, a
    /// prefix somebody typed, a path read back out of a record — so none of
    /// them is a fact this Library minted and none of them is written down.
    /// What is left is the shape of the refusal, which is what a reader
    /// grouping a log file by it is after.
    fn redacted(&self) -> String {
        match self {
            Self::InvalidHexLength { expected, actual } => {
                format!("Model::InvalidHexLength(expected={expected}, actual={actual})")
            }
            Self::InvalidHexDigit { .. } => "Model::InvalidHexDigit".to_owned(),
            Self::InvalidByteLength { expected, actual } => {
                format!("Model::InvalidByteLength(expected={expected}, actual={actual})")
            }
            Self::EpochOutOfRange => "Model::EpochOutOfRange".to_owned(),
            Self::GenerationOutOfRange => "Model::GenerationOutOfRange".to_owned(),
            Self::InvalidReplicaPosition { index, count } => {
                format!("Model::InvalidReplicaPosition(index={index}, count={count})")
            }
            Self::MalformedObjectName { .. } => "Model::MalformedObjectName".to_owned(),
            Self::InvalidSetDigest { .. } => "Model::InvalidSetDigest".to_owned(),
            Self::MalformedPrefixBase { .. } => "Model::MalformedPrefixBase".to_owned(),
            Self::InvalidReplicaCount => "Model::InvalidReplicaCount".to_owned(),
            // The two variants carrying a path, and the reason this whole
            // vocabulary needs a rendering of its own: it is Library content
            // whatever it turns out to be a path to, so only its length
            // survives — beside the defect, which says what is wrong with the
            // path without saying any of it.
            Self::UnnormalizedEntryPath { path } => {
                format!("Model::UnnormalizedEntryPath(path_len={})", path.len())
            }
            Self::MalformedEntryPath { path, defect } => format!(
                "Model::MalformedEntryPath(defect={defect}, path_len={})",
                path.len()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // EP-1: a path read back out of a record is the user's own name for their
    // file whether or not it is spelled the way the Library spells one, so the
    // message that names it is not what a log line renders.
    #[test]
    fn a_path_a_record_carried_never_reaches_a_log_line() {
        let error = Error::UnnormalizedEntryPath {
            path: "albums/spring.jpg".to_owned(),
        };

        assert!(error.to_string().contains("albums/spring.jpg"));
        assert_eq!(
            error.redacted(),
            "Model::UnnormalizedEntryPath(path_len=17)",
        );
    }

    // EP-2: the defect is what a reader grouping a log file by refusal is
    // after, and it says which part of the shape went without saying any of the
    // path it went in.
    #[test]
    fn a_malformed_path_reaches_a_log_line_as_its_defect_alone() {
        let error = Error::MalformedEntryPath {
            path: "albums/../etc".to_owned(),
            defect: PathDefect::RelativeComponent,
        };

        assert!(error.to_string().contains("albums/../etc"));
        assert_eq!(
            error.redacted(),
            "Model::MalformedEntryPath(defect=it holds a `.` or `..` component, path_len=13)",
        );
    }

    #[test]
    fn a_refusal_about_a_name_keeps_its_identity_and_drops_the_name() {
        let error = Error::MalformedObjectName {
            name: "not-a-control-object".to_owned(),
        };

        assert_eq!(error.redacted(), "Model::MalformedObjectName");
    }
}
