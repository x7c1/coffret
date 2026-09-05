use crate::error::{Error, Result};
use crate::format_integer::MAX_FORMAT_INTEGER;

/// How long a Journal record said a Container's ciphertext is (spec: CP-11,
/// FM-15, FM-16).
///
/// A claim, and the name says so because nothing checks it. What proves a
/// Container's ciphertext is the ciphertext its Library committed is the
/// BLAKE3-256 the same record carries and the authenticated chunks the object
/// is made of (spec: FM-5, FM-8, FM-15) — a length is neither, and a device
/// that trusted this number over those would be taking Storage's word for the
/// object. So it is carried for what it is worth: a figure to log, and a figure
/// the device that wrote the Container can compare its own measurement against.
///
/// What is refused is the one thing the format says about the number whatever
/// it is worth: it is at most [`MAX_FORMAT_INTEGER`], as every integer a
/// control payload carries is (spec: FM-19). Past that it is not a claim this
/// format can spell, so it is not one this type will hold. What the type adds
/// beyond that is that a caller reaching for the number has to say it is
/// reaching for a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CiphertextLenClaim(u64);

impl CiphertextLenClaim {
    /// The claim a record makes, or a writer makes about the object it just
    /// wrote.
    ///
    /// # Errors
    ///
    /// [`Error::CiphertextLenOutOfRange`] where `len` is past
    /// [`MAX_FORMAT_INTEGER`] (spec: FM-19).
    pub fn new(len: u64) -> Result<Self> {
        if len > MAX_FORMAT_INTEGER {
            return Err(Error::CiphertextLenOutOfRange { len });
        }
        Ok(Self(len))
    }

    /// The number claimed, for a caller that has decided what it is worth.
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // FM-19: a control payload's integers are below 2^63, and a ciphertext
    // length is one of them — so the bound itself is a claim and the number
    // above it is not.
    #[test]
    fn a_claim_past_the_formats_integer_range_is_refused() {
        let claim = CiphertextLenClaim::new(MAX_FORMAT_INTEGER).expect("the bound is a claim");
        assert_eq!(claim.get(), MAX_FORMAT_INTEGER);

        let result = CiphertextLenClaim::new(MAX_FORMAT_INTEGER + 1);
        assert!(
            matches!(
                result,
                Err(Error::CiphertextLenOutOfRange { len }) if len == MAX_FORMAT_INTEGER + 1
            ),
            "expected 2^63 to be no ciphertext length the format spells, got {result:?}"
        );
    }
}
