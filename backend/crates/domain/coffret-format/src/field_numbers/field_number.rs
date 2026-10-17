use ciborium::Value;

/// A CBOR map key as the field number it carries, if it is an unsigned
/// integer.
pub(crate) fn field_number(key: &Value) -> Option<u64> {
    key.as_integer()
        .and_then(|integer| u64::try_from(integer).ok())
}
