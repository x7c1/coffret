use std::error;
use std::fmt;

use coffret_model::{lowercase_hex, ContentHash, EntryPath, Redacted};

/// Why a Container could not be read and verified for read-modify-replace
/// (spec: PK-10).
///
/// Every one of these is a verdict about the object or the catalog rather than
/// about the transfer: a provider that answered short or failed outright is
/// retried and, if it keeps failing, fails the run instead. What is here is a
/// Container that came back whole and was not what the Library says it is — so
/// no replacement is written from it, because a replacement is a claim that it
/// carries the old Entries forward and nothing read here can back that claim.
///
/// There is deliberately no `PartialEq`: a caller decides from the variant and
/// the fields it names.
#[derive(Debug)]
pub enum Unverified {
    /// Storage holds no object for the Container, so there is nothing to read
    /// (spec: FM-3).
    Unreachable,
    /// The object hashes to something other than what the Journal record
    /// recorded for it (spec: FM-15, CP-11).
    CiphertextMismatch {
        /// What the record says the object hashes to.
        expected: ContentHash,
        /// What the object that came back hashes to.
        actual: ContentHash,
    },
    /// The object would not open: its key would not unwrap, or its header, meta
    /// section, or a chunk failed to authenticate or ended early
    /// (spec: FM-2, FM-5, FM-8, FM-14).
    Unopenable(coffret_format::Error),
    /// The Container's own meta section does not describe what the catalog
    /// records for it — another kind, or another entry table (spec: CP-11,
    /// FM-9).
    ///
    /// The meta section is the authority on what a Container holds, and the
    /// record is a copy of it; a disagreement is a catalog that cannot be
    /// trusted to say which Entries a replacement should carry.
    Disagrees,
    /// One Entry's plaintext does not hash to what its row records
    /// (spec: CP-11, PK-10).
    EntryMismatch {
        /// The Entry whose content is not the content its row names.
        path: EntryPath,
    },
}

impl fmt::Display for Unverified {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable => f.write_str("Storage holds no object for the Container"),
            Self::CiphertextMismatch { expected, actual } => write!(
                f,
                "the Container's object hashes to {}, and its record names {}",
                lowercase_hex::encode(actual.as_bytes()),
                lowercase_hex::encode(expected.as_bytes()),
            ),
            // What the format layer refused is left to `source`, for the reason
            // every wrapper in this crate leaves its cause there.
            Self::Unopenable(_) => f.write_str("the Container would not open"),
            Self::Disagrees => f.write_str(
                "the Container's own entry table is not the one the catalog records for it",
            ),
            // The path stays in the value, for the reason every Entry Path in an
            // error does: a message is the part most likely to be logged.
            Self::EntryMismatch { .. } => {
                f.write_str("an Entry's content is not the content its row records")
            }
        }
    }
}

impl error::Error for Unverified {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Unopenable(error) => Some(error),
            _ => None,
        }
    }
}

impl Redacted for Unverified {
    fn redacted(&self) -> String {
        match self {
            Self::Unreachable => "Unverified::Unreachable".to_owned(),
            Self::CiphertextMismatch { .. } => "Unverified::CiphertextMismatch".to_owned(),
            Self::Unopenable(error) => format!("Unverified::Unopenable: {}", error.redacted()),
            Self::Disagrees => "Unverified::Disagrees".to_owned(),
            Self::EntryMismatch { path } => {
                format!(
                    "Unverified::EntryMismatch(path_len={})",
                    path.as_str().len()
                )
            }
        }
    }
}
