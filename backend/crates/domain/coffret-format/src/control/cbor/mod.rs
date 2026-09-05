//! Reading and writing the CBOR maps a control-object payload is made of.
//!
//! The payload schemas (FM-15, FM-16, FM-17) are maps with text keys whose
//! fields are read one at a time rather than deserialized as a whole struct,
//! for two reasons. A field that is not the shape its rule gives it has to be
//! reported as *that field* — `MalformedJournalRecord { detail }` naming the
//! key — and two of the maps are a shared map plus one field of their own (an
//! addition is a Container plus its entry table, a Snapshot entry is a catalog
//! entry map (FM-16) plus its `container` index), which a struct would either
//! duplicate or flatten.
//!
//! Which map is being read only changes the error a malformed field raises, so
//! each reader takes that constructor and everything else here is shared.

use std::fmt::Display;

use ciborium::Value;
use coffret_model::MAX_FORMAT_INTEGER;

use crate::error::{Error, Result};
use crate::malformed_cbor::malformed_cbor;

mod fields;
pub(super) use fields::Fields;

mod map_builder;
pub(super) use map_builder::MapBuilder;

/// The field every payload schema states its version in (FM-9, FM-15, FM-16,
/// FM-17).
pub(super) const SCHEMA_FIELD: &str = "schema";

/// Serializes a payload body map to the CBOR bytes the framing seals.
///
/// The framing adds `master_key_epoch` and the padding around what this
/// returns (FM-11, FM-13), so the bytes here are the kind's own map alone.
pub(super) fn write_body(value: &Value) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes).map_err(serialization_failed)?;
    Ok(bytes)
}

/// Reads a payload body back as one CBOR item, rejecting anything after it.
///
/// The framing has already taken the padding off (FM-11), so a body with bytes
/// trailing its map is one no writer following the rule produced.
pub(super) fn read_body(bytes: &[u8], malformed: fn(String) -> Error) -> Result<Value> {
    let mut remaining = bytes;
    let value: Value =
        ciborium::from_reader(&mut remaining).map_err(|error| malformed_cbor(error, malformed))?;
    if !remaining.is_empty() {
        return Err(malformed(format!(
            "{} bytes follow the payload map",
            remaining.len()
        )));
    }
    Ok(value)
}

/// One CBOR item read as an unsigned integer the format admits (FM-19).
///
/// `None` covers both ways an item is not one: it is not a CBOR integer that
/// is zero or above, or it is one the format does not carry — every unsigned
/// integer a control payload spells is below 2^63. Stating the bound here is
/// what keeps the payload readers from each restating it: `schema`, `prev`,
/// the generations, `master_key_epoch`, `ciphertext_len`, and `container` are
/// all read through this.
pub(super) fn as_bounded_uint(value: &Value) -> Option<u64> {
    value
        .as_integer()
        .and_then(|integer| u64::try_from(integer).ok())
        .filter(|number| *number <= MAX_FORMAT_INTEGER)
}

/// Reports a value this crate built that CBOR would not take.
pub(super) fn serialization_failed(error: impl Display) -> Error {
    Error::ControlPayloadEncodeFailed {
        detail: error.to_string(),
    }
}

/// A CBOR value that is not the struct a schema spells, as the malformed
/// payload of whichever schema was reading it.
///
/// `value::Error` displays itself in its `Debug` spelling, so passing it
/// through `to_string` would reach a caller as `Custom("…")` with the message
/// quoted inside it. The message is the whole of what the error carries — the
/// field a deserializer refused, and what it expected there — so it is taken on
/// its own. Which map was being read is `malformed`, as it is everywhere else
/// here.
pub(super) fn deserialization_failed(
    error: ciborium::value::Error,
    malformed: fn(String) -> Error,
) -> Error {
    match error {
        ciborium::value::Error::Custom(message) => malformed(message),
    }
}

/// Names the CBOR item a field carries, for a message about the field.
///
/// It never quotes the item, for the reason [`Error`] gives.
pub(super) fn describe(value: &Value) -> &'static str {
    match value {
        Value::Integer(_) => "an integer",
        Value::Bytes(_) => "a byte string",
        Value::Float(_) => "a float",
        Value::Text(_) => "text",
        Value::Bool(_) => "a boolean",
        Value::Null => "null",
        Value::Tag(_, _) => "a tagged item",
        Value::Array(_) => "an array",
        Value::Map(_) => "a map",
        _ => "an item of an unknown type",
    }
}
