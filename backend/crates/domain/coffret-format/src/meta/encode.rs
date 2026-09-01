use coffret_model::{ContainerKind, EntryMetadata};

use super::wire_kind::WireKind;
use super::wire_meta::WireMeta;
use super::wire_meta_entry::WireMetaEntry;
use super::{Meta, SCHEMA};
use crate::error::{Error, Result};

/// Serializes a meta section to its CBOR plaintext.
pub(crate) fn encode(meta: &Meta) -> Result<Vec<u8>> {
    write(&WireMeta {
        schema: SCHEMA,
        kind: WireKind::from(meta.kind),
        pad_len: meta.pad_len,
        entries: meta.entries.iter().map(WireMetaEntry::from).collect(),
    })
}

/// How many CBOR bytes one row of the entry table occupies.
///
/// Every row is a map of its own, so the table's length is the sum of its rows
/// — which is what lets a footprint be extended one Entry at a time instead of
/// re-serializing the whole table at every step of a segmentation
/// (spec: PK-3, PK-6).
pub(crate) fn entry_len(entry: &EntryMetadata) -> Result<u64> {
    Ok(write(&WireMetaEntry::from(entry))?.len() as u64)
}

/// How many CBOR bytes the meta map occupies apart from its rows.
///
/// The Container-level fields, plus the array header the rows sit under. That
/// header widens with the count and `pad_len` widens with the stream, so both
/// are asked for rather than assumed.
pub(crate) fn envelope_len(kind: ContainerKind, pad_len: u64, count: usize) -> Result<u64> {
    let empty = write(&WireMeta {
        schema: SCHEMA,
        kind: WireKind::from(kind),
        pad_len,
        entries: Vec::new(),
    })?
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

/// One CBOR value as the bytes a writer would produce.
fn write<T: serde::Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes).map_err(|error| Error::MetaEncodeFailed {
        detail: error.to_string(),
    })?;
    Ok(bytes)
}
