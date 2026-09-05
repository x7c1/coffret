use std::fmt::Debug;

use crate::error::Error;

/// ciborium's account of bytes that are not the CBOR an object's rule spells.
///
/// A semantic error is the one that says something a caller can act on — which
/// field a deserializer refused, and what it was expecting there — and
/// ciborium's `Display` is its `Debug` spelling, which would reach the caller
/// as `Semantic(None, "…")` with that message quoted inside it. The message is
/// taken on its own; the remaining variants are ciborium's own syntax, recursion
/// and I/O errors, which carry no inner message to prefer.
///
/// `malformed` is what the carrier calls such bytes: a meta section (FM-9) and
/// a control payload (FM-11) each have their own variant, and each schema read
/// out of a payload body has one again, so the reading is stated once here and
/// the constructor says which object was being read — as it does for
/// [`crate::bounded_uint::bounded_uint`].
pub(crate) fn malformed_cbor<E: Debug>(
    error: ciborium::de::Error<E>,
    malformed: fn(String) -> Error,
) -> Error {
    match error {
        ciborium::de::Error::Semantic(_, message) => malformed(message),
        other => malformed(other.to_string()),
    }
}
