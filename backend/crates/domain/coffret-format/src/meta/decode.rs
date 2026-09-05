use std::fmt::Debug;

use super::malformed;
use super::wire_meta::WireMeta;
use super::wire_meta_entry::WireMetaEntry;
use super::Meta;
use super::SCHEMA;
use crate::bounded_uint::bounded_uint;
use crate::error::{Error, Result};
use crate::padme;

/// Parses a meta section from its CBOR plaintext and validates the entry table.
///
/// The plaintext is one CBOR map followed by a zero-filled padding tail, so
/// this reads exactly one item and then holds what is left to FM-9's padding
/// rule: exactly the zero bytes that carry the map to its Padmé bucket. A
/// non-zero byte would make the padding a place to ride bytes past a reader,
/// and any other length was written by something that did not pad as the rule
/// says — which would leave the header's meta section length (FM-2) saying
/// something the map does not.
pub(crate) fn decode(bytes: &[u8]) -> Result<Meta> {
    let mut padding = bytes;
    let wire: WireMeta = ciborium::from_reader(&mut padding).map_err(not_this_map)?;

    let map_len = (bytes.len() - padding.len()) as u64;
    let expected = padme::padded_len(map_len);
    if expected != bytes.len() as u64 {
        return Err(Error::MetaPaddingLengthMismatch {
            expected,
            actual: bytes.len() as u64,
        });
    }
    if padding.iter().any(|byte| *byte != 0) {
        return Err(Error::NonZeroMetaPadding);
    }
    // The Container-level numbers, held to the bound FM-19 puts on every
    // integer the format carries. The struct's fields are plain `u64`s, which
    // is the whole 64-bit range, so this is where the map's own arithmetic
    // becomes numbers the format admits — and the field that carried one past
    // the bound is named, the way a payload's fields already are.
    let schema = bounded_uint("schema", wire.schema, malformed)?;
    let pad_len = bounded_uint("pad_len", wire.pad_len, malformed)?;

    if schema < SCHEMA {
        return Err(Error::UnsupportedMetaSchema { schema });
    }
    if wire.entries.is_empty() {
        return Err(Error::EmptyEntryTable);
    }
    let entries = wire
        .entries
        .iter()
        .map(WireMetaEntry::to_metadata)
        .collect::<Result<Vec<_>>>()?;

    // The entries must tile the stream from zero without gaps or overlaps:
    // that is what makes an Entry's extent usable to range-read it. Each extent
    // already ends inside the stream's address space — that is what building
    // one refused on (FM-19) — so the walk asks only where they sit relative to
    // one another.
    let mut expected_offset = 0u64;
    for (index, entry) in entries.iter().enumerate() {
        if entry.extent.offset() != expected_offset {
            return Err(Error::EntryTableNotContiguous { index });
        }
        expected_offset = entry.extent.end();
    }

    Ok(Meta {
        kind: wire.kind.into(),
        pad_len,
        entries,
    })
}

/// ciborium's account of a plaintext that is not the map FM-9 spells.
///
/// A semantic error is the one that says something a caller can act on — which
/// field a deserializer refused, and what it was expecting there — and
/// ciborium's `Display` is its `Debug` spelling, which would reach the caller
/// as `Semantic(None, "…")` with that message quoted inside it. The message is
/// taken on its own; the remaining variants are ciborium's own syntax, recursion
/// and I/O errors, which carry no inner message to prefer.
fn not_this_map<E: Debug>(error: ciborium::de::Error<E>) -> Error {
    match error {
        ciborium::de::Error::Semantic(_, message) => malformed(message),
        other => malformed(other.to_string()),
    }
}
