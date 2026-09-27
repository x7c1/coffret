//! What a malformed object's variant says went wrong, and whose words those are.

use std::fmt;

use super::CborDecodeFailure;

/// Why an object is not the CBOR shape its rule spells.
///
/// Two provenances, kept apart by type because the crate's policy treats them
/// differently (see [`Error`](super::Error)). A sentence this crate composed
/// about the bytes it read is [`Written`](Self::Written), and its variant's
/// `Display` carries it. The CBOR decoder's own refusal is
/// [`Undecodable`](Self::Undecodable): it travels as the cause a caller reaches
/// through `source()`, and the variant's own line stops at which object it was.
#[derive(Debug, Clone)]
pub enum MalformedDetail {
    /// Which field, and what was found there instead, in this crate's words.
    Written(String),
    /// The CBOR decoder refused the bytes, and this is its account.
    Undecodable(CborDecodeFailure),
}

impl MalformedDetail {
    /// The decoder's refusal, where it is what this detail is.
    pub(crate) fn cause(&self) -> Option<&CborDecodeFailure> {
        match self {
            Self::Written(_) => None,
            Self::Undecodable(cause) => Some(cause),
        }
    }
}

/// Whichever account the detail is, as its text.
///
/// For a reader asking what the detail says; a variant's own `Display` writes
/// only the [`Written`](MalformedDetail::Written) half.
impl fmt::Display for MalformedDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Written(detail) => f.write_str(detail),
            Self::Undecodable(cause) => cause.fmt(f),
        }
    }
}
