use coffret_model::{ContainerKind, EntryMetadata};

use super::wire_kind::WireKind;
use super::wire_meta::WireMeta;
use super::wire_meta_entry::WireMetaEntry;
use super::{Meta, SCHEMA};
use crate::bounded_uint::bounded_uint;
use crate::error::{CborEncodeFailure, EncodeDetail, Error, Result};
use crate::field_numbers::{self, MapFields};

/// Serializes a meta section to its CBOR plaintext.
///
/// The padding length is held to the bound FM-19 puts on every integer the
/// format carries, so nothing this crate writes is a map its own reader would
/// refuse. It is the one number here that needs saying, for the reason
/// `bounded_uint` gives.
pub(crate) fn encode(meta: &Meta) -> Result<Vec<u8>> {
    write(
        &WireMeta {
            schema: SCHEMA,
            kind: WireKind::from(meta.kind),
            pad_len: bounded_uint("pad_len", meta.pad_len, encode_failed)?,
            entries: meta.entries.iter().map(WireMetaEntry::from).collect(),
        },
        &field_numbers::META,
    )
}

/// How many CBOR bytes one row of the entry table occupies.
///
/// Every row is a map of its own, so the table's length is the sum of its rows
/// — which is what lets a footprint be extended one Entry at a time instead of
/// re-serializing the whole table at every step of a segmentation
/// (spec: PK-3, PK-6).
pub(crate) fn entry_len(entry: &EntryMetadata) -> Result<u64> {
    Ok(write(&WireMetaEntry::from(entry), &field_numbers::META_ENTRY)?.len() as u64)
}

/// How many CBOR bytes the meta map occupies apart from its rows.
///
/// The Container-level fields, plus the array header the rows sit under. That
/// header widens with the count and `pad_len` widens with the stream, so both
/// are asked for rather than assumed.
pub(crate) fn envelope_len(kind: ContainerKind, pad_len: u64, count: usize) -> Result<u64> {
    let empty = write(
        &WireMeta {
            schema: SCHEMA,
            kind: WireKind::from(kind),
            pad_len: bounded_uint("pad_len", pad_len, encode_failed)?,
            entries: Vec::new(),
        },
        &field_numbers::META,
    )?
    .len() as u64;
    // What that counted for the rows is the single byte an empty array is.
    Ok(empty - 1 + array_header_len(count))
}

/// How many bytes CBOR spends saying how long an array is.
const fn array_header_len(count: usize) -> u64 {
    match count as u64 {
        0..=23 => 1,
        24..=0xff => 2,
        0x100..=0xffff => 3,
        0x1_0000..=0xffff_ffff => 5,
        _ => 9,
    }
}

/// One of the meta section's maps as the bytes a writer would produce, every
/// field under the field number `kind` assigns it (spec: FM-9).
///
/// The structs spell the fields by name, which is how this crate and the
/// specification speak of them; the names become field numbers here, on the
/// way out.
fn write<T: serde::Serialize>(value: &T, kind: &MapFields) -> Result<Vec<u8>> {
    let named = ciborium::Value::serialized(value)
        .map_err(|error| unencodable(CborEncodeFailure::converting(error)))?;
    let wire = field_numbers::to_wire(named, kind)?;
    let mut bytes = Vec::new();
    ciborium::into_writer(&wire, &mut bytes)
        .map_err(|error| unencodable(CborEncodeFailure::writing(error)))?;
    Ok(bytes)
}

/// What a meta section holding a number this crate will not write is reported
/// as.
fn encode_failed(detail: String) -> Error {
    Error::MetaEncodeFailed {
        detail: EncodeDetail::Written(detail),
    }
}

/// What a meta section the CBOR encoder would not take is reported as.
fn unencodable(cause: CborEncodeFailure) -> Error {
    Error::MetaEncodeFailed {
        detail: EncodeDetail::Unencodable(cause),
    }
}
