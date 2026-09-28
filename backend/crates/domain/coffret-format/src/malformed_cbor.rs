use std::io;

use crate::error::{CborDecodeFailure, Error, MalformedDetail};

/// ciborium's refusal of bytes that are not the CBOR an object's rule spells,
/// as that object's malformed variant.
///
/// `malformed` is what the carrier calls such bytes: a meta section
/// (spec: FM-9) and a control payload (spec: FM-11) each have their own
/// variant, and each schema read out of a payload body has one again, so the
/// reading is stated once here and the constructor says which object was being
/// read — as it does for [`crate::bounded_uint::bounded_uint`]. The refusal
/// itself travels as the value ciborium reported, never as its text (see
/// [`Error`]).
pub(crate) fn malformed_cbor(
    error: ciborium::de::Error<io::Error>,
    malformed: fn(MalformedDetail) -> Error,
) -> Error {
    malformed(MalformedDetail::Undecodable(CborDecodeFailure::reading(
        error,
    )))
}
