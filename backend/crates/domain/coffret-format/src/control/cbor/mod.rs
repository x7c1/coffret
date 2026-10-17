//! Reading and writing the CBOR maps a control-object payload is made of.
//!
//! The payload schemas (spec: FM-15, FM-16, FM-17) are maps whose fields travel
//! under the field numbers the field-number registry assigns them, and which
//! are read one field at a time rather than deserialized as a whole struct, for
//! two reasons.
//! A field that is not the shape its rule gives it has to be reported as *that
//! field* — `MalformedJournalRecord { detail }` naming it — and two of the maps
//! are a shared map plus fields of their own (an addition is a Container plus
//! its entry table, a Snapshot entry is a catalog entry map (spec: FM-16) plus
//! its `container` index and `entry` number), which a struct would either
//! duplicate or flatten.
//!
//! The readers and builders here speak field names throughout. The body is
//! turned from names into field numbers as it is written and back as it is read
//! ([`crate::field_numbers`]), so every refusal names the field the way the
//! specification does.
//!
//! Which map is being read only changes the error a malformed field raises, so
//! each reader takes that constructor and everything else here is shared.

use ciborium::Value;
use coffret_model::{EntryNumber, MAX_FORMAT_INTEGER};

use crate::error::{CborDecodeFailure, CborEncodeFailure, Error, MalformedDetail, Result};
use crate::field_numbers::{self, MapFields};
use crate::malformed_cbor::malformed_cbor;

mod fields;
pub(super) use fields::Fields;

mod map_builder;
pub(super) use map_builder::MapBuilder;

/// The field every payload schema states its version in
/// (spec: FM-9, FM-15, FM-16, FM-17).
pub(super) const SCHEMA_FIELD: &str = "schema";

/// Serializes a payload body map, keyed by field names, to the CBOR bytes the
/// framing seals, every field under the field number `kind` assigns it.
///
/// The framing adds `master_key_epoch` and the padding around what this returns
/// (spec: FM-11, FM-13), so the bytes here are the kind's own map alone.
pub(super) fn write_body(value: Value, kind: &MapFields) -> Result<Vec<u8>> {
    let wire = field_numbers::to_wire(value, kind)?;
    let mut bytes = Vec::new();
    ciborium::into_writer(&wire, &mut bytes).map_err(|error| {
        Error::ControlPayloadEncodeFailed {
            cause: CborEncodeFailure::writing(error),
        }
    })?;
    Ok(bytes)
}

/// Reads a payload body back as one CBOR item keyed by field names, rejecting
/// anything after it.
///
/// The framing has already taken the padding off (spec: FM-11), so a body with
/// bytes trailing its map is one no writer following the rule produced. Map
/// keys `kind` does not name — unknown field numbers, retired ones, and text —
/// are dropped on the way, so a field carried under anything but its field
/// number reaches the reader as missing.
pub(super) fn read_body(
    bytes: &[u8],
    malformed: fn(MalformedDetail) -> Error,
    kind: &MapFields,
) -> Result<Value> {
    let mut remaining = bytes;
    let value: Value =
        ciborium::from_reader(&mut remaining).map_err(|error| malformed_cbor(error, malformed))?;
    if !remaining.is_empty() {
        return Err(malformed(MalformedDetail::Written(format!(
            "{} bytes follow the payload map",
            remaining.len()
        ))));
    }
    Ok(field_numbers::from_wire(value, kind))
}

/// One CBOR item read as an unsigned integer the format admits (spec: FM-19).
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

/// The field at `key` read as an entry number (spec: FM-9), or the malformed
/// payload of whichever schema was reading it.
///
/// Both schemas that name an Entry by its position — a rename in a Journal
/// record (spec: FM-15) and an Entry in an Index Snapshot (spec: FM-16) — read
/// it through here, so the 32-bit bound is reported the same way in both.
pub(super) fn entry_number(
    fields: &Fields<'_>,
    key: &str,
    malformed: fn(String) -> Error,
) -> Result<EntryNumber> {
    let number = fields.uint(key)?;
    EntryNumber::new(number).map_err(|_| {
        malformed(format!(
            "{key} is an entry number below 2^32, found {number}"
        ))
    })
}

/// Reports a struct this crate built that CBOR would not turn into an item,
/// carried as the value ciborium reported.
pub(super) fn serialization_failed(error: ciborium::value::Error) -> Error {
    Error::ControlPayloadEncodeFailed {
        cause: CborEncodeFailure::converting(error),
    }
}

/// A CBOR value that is not the struct a schema spells, as the malformed
/// payload of whichever schema was reading it.
///
/// The refusal travels as the value ciborium reported, which says which field
/// a deserializer refused and what it expected there. Which map was being read
/// is `malformed`, as it is everywhere else here.
pub(super) fn deserialization_failed(
    error: ciborium::value::Error,
    malformed: fn(MalformedDetail) -> Error,
) -> Error {
    malformed(MalformedDetail::Undecodable(CborDecodeFailure::converting(
        error,
    )))
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
