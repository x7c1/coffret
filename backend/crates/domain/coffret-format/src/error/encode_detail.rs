//! What an object this crate could not write says went wrong, and whose words
//! those are.

use std::fmt;

use super::CborEncodeFailure;

/// Why a value this crate built could not be written.
///
/// Two provenances, kept apart by type the way
/// [`MalformedDetail`](super::MalformedDetail) keeps them on the way in. A
/// sentence this crate composed about the value it refused to write is
/// [`Written`](Self::Written), and its variant's `Display` carries it. The CBOR
/// encoder's own refusal is [`Unencodable`](Self::Unencodable): it travels as
/// the cause a caller reaches through `source()`, and the variant's own line
/// stops at which object it was.
#[derive(Debug, Clone)]
pub enum EncodeDetail {
    /// What this crate refused to write, in its own words.
    Written(String),
    /// The CBOR encoder refused the value, and this is its account.
    Unencodable(CborEncodeFailure),
}

impl EncodeDetail {
    /// The encoder's refusal, where it is what this detail is.
    pub(crate) fn cause(&self) -> Option<&CborEncodeFailure> {
        match self {
            Self::Written(_) => None,
            Self::Unencodable(cause) => Some(cause),
        }
    }
}

/// Whichever account the detail is, as its text.
///
/// For a reader asking what the detail says; a variant's own `Display` writes
/// only the [`Written`](EncodeDetail::Written) half.
impl fmt::Display for EncodeDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Written(detail) => f.write_str(detail),
            Self::Unencodable(cause) => cause.fmt(f),
        }
    }
}
