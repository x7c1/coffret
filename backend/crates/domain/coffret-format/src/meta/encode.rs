use coffret_model::{ContainerKind, EntryMetadata};

use super::wire_entry::WireEntry;
use super::wire_kind::WireKind;
use super::wire_meta::WireMeta;
use super::{Meta, SCHEMA};
use crate::error::{Error, Result};

/// Serializes a meta section to its CBOR plaintext.
pub(crate) fn encode(meta: &Meta) -> Result<Vec<u8>> {
    write(&WireMeta {
        schema: SCHEMA,
        kind: WireKind::from(meta.kind),
        pad_len: meta.pad_len,
        entries: meta.entries.iter().map(WireEntry::from).collect(),
    })
}

/// How many CBOR bytes one row of the entry table occupies.
///
/// Every row is a map of its own, so the table's length is the sum of its rows
/// — which is what lets a footprint be extended one Entry at a time instead of
/// re-serializing the whole table at every step of a segmentation
/// (spec: PK-3, PK-6).
pub(crate) fn entry_len(entry: &EntryMetadata) -> Result<u64> {
    Ok(write(&WireEntry::from(entry))?.len() as u64)
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

#[cfg(test)]
mod tests {
    use coffret_model::ContainerKind;

    use super::super::testing::{as_value, sample};

    // FM-9: the meta section is one CBOR map with `schema`, `kind`, `pad_len`,
    // and `entries`; each entry records `path`, `offset`, `size`, `mtime`, and
    // `hash`, plus optional `derived_from` and `mime`.
    #[test]
    fn field_names_and_kind_spelling_match_the_rule() {
        let value = as_value(&sample());
        let map = value.as_map().expect("the meta section is a CBOR map");
        let keys: Vec<&str> = map
            .iter()
            .map(|(key, _)| key.as_text().expect("keys are text"))
            .collect();
        assert_eq!(keys, ["schema", "kind", "pad_len", "entries"]);

        let container_kind = map
            .iter()
            .find(|(key, _)| key.as_text() == Some("kind"))
            .map(|(_, value)| value)
            .expect("kind is present");
        assert_eq!(container_kind.as_text(), Some("pack"));

        let entries = map
            .iter()
            .find(|(key, _)| key.as_text() == Some("entries"))
            .map(|(_, value)| value.as_array().expect("entries is an array"))
            .expect("entries is present");
        let entry_keys: Vec<&str> = entries[0]
            .as_map()
            .expect("an entry is a CBOR map")
            .iter()
            .map(|(key, _)| key.as_text().expect("keys are text"))
            .collect();
        assert_eq!(entry_keys, ["path", "offset", "size", "mtime", "hash"]);
    }

    // FM-9: the other spelling `kind` carries is `one-file`.
    #[test]
    fn one_file_kind_spelling_matches_the_rule() {
        let mut meta = sample();
        meta.kind = ContainerKind::OneFile;
        let value = as_value(&meta);
        let map = value.as_map().expect("the meta section is a CBOR map");
        let container_kind = map
            .iter()
            .find(|(key, _)| key.as_text() == Some("kind"))
            .map(|(_, value)| value)
            .expect("kind is present");
        assert_eq!(container_kind.as_text(), Some("one-file"));
    }
}
