//! The one place this crate's tests turn a literal number into a
//! [`CiphertextLenClaim`].
//!
//! A claim is a number the format bounds (spec: FM-19), so the type has no
//! constructor that takes one on trust; a fixture is no exception. The unwrap
//! lives here rather than at each of the fixtures so that a number somebody
//! mistypes is reported once, as the mistake in the fixture that it is.

use coffret_model::CiphertextLenClaim;

/// The ciphertext length `len` claims, or a panic naming the literal that
/// claims none.
pub(crate) fn ciphertext_len(len: u64) -> CiphertextLenClaim {
    CiphertextLenClaim::new(len)
        .unwrap_or_else(|error| panic!("a fixture holds a literal ciphertext length: {error}"))
}
