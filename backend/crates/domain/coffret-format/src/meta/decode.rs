use super::wire_entry::WireEntry;
use super::wire_meta::WireMeta;
use super::Meta;
use super::SCHEMA;
use crate::error::{Error, Result};

/// Parses a meta section from its CBOR plaintext and validates the entry table.
pub(crate) fn decode(bytes: &[u8]) -> Result<Meta> {
    let wire: WireMeta = ciborium::from_reader(bytes).map_err(|error| Error::MalformedMeta {
        detail: error.to_string(),
    })?;
    if wire.schema < SCHEMA {
        return Err(Error::UnsupportedMetaSchema {
            schema: wire.schema,
        });
    }
    if wire.entries.is_empty() {
        return Err(Error::EmptyEntryTable);
    }
    let entries = wire
        .entries
        .iter()
        .map(WireEntry::to_metadata)
        .collect::<Result<Vec<_>>>()?;

    // The entries must tile the stream from zero without gaps or overlaps:
    // that is what makes `offset` and `size` usable to range-read one Entry.
    let mut expected_offset = 0u64;
    for (index, entry) in entries.iter().enumerate() {
        if entry.offset != expected_offset {
            return Err(Error::EntryTableNotContiguous { index });
        }
        expected_offset = entry
            .offset
            .checked_add(entry.size)
            .ok_or(Error::StreamTooLong)?;
    }

    Ok(Meta {
        kind: wire.kind.into(),
        pad_len: wire.pad_len,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use ciborium::Value;
    use coffret_model::{ContainerId, ContainerKind, DerivedFrom, EntryPath};

    use super::super::encode;
    use super::super::testing::{as_value, entry, sample, to_bytes};
    use super::*;

    #[test]
    fn optional_fields_round_trip() {
        let mut meta = sample();
        meta.entries[0].mime = Some("text/plain".to_owned());
        meta.entries[0].derived_from = Some(DerivedFrom {
            container_id: ContainerId::from_bytes([3u8; ContainerId::BYTE_LEN]),
            path: EntryPath::new("originals/a.txt"),
        });
        let decoded =
            decode(&encode(&meta).expect("encoding succeeds")).expect("decoding succeeds");
        assert_eq!(decoded.entries, meta.entries);
    }

    // FM-9: the maps are forward-open — a reader ignores fields it does not
    // know, so a newer writer can add fields without breaking this reader.
    #[test]
    fn unknown_fields_are_ignored() {
        let Value::Map(mut map) = as_value(&sample()) else {
            panic!("the meta section is a CBOR map");
        };
        map.push((
            Value::Text("future_field".to_owned()),
            Value::Text("whatever".to_owned()),
        ));
        for (key, value) in map.iter_mut() {
            if key.as_text() != Some("entries") {
                continue;
            }
            let Value::Array(entries) = value else {
                panic!("entries is an array");
            };
            for entry in entries {
                let Value::Map(fields) = entry else {
                    panic!("an entry is a CBOR map");
                };
                fields.push((
                    Value::Text("future_entry_field".to_owned()),
                    Value::Integer(1i64.into()),
                ));
            }
        }
        // A newer writer would also have bumped `schema`.
        for (key, value) in map.iter_mut() {
            if key.as_text() == Some("schema") {
                *value = Value::Integer(2.into());
            }
        }

        let decoded = decode(&to_bytes(&Value::Map(map))).expect("unknown fields are ignored");
        assert_eq!(decoded.entries, sample().entries);
        assert_eq!(decoded.pad_len, 7);
    }

    #[test]
    fn schema_zero_is_rejected() {
        let Value::Map(mut map) = as_value(&sample()) else {
            panic!("the meta section is a CBOR map");
        };
        for (key, value) in map.iter_mut() {
            if key.as_text() == Some("schema") {
                *value = Value::Integer(0.into());
            }
        }
        assert_eq!(
            decode(&to_bytes(&Value::Map(map))),
            Err(Error::UnsupportedMetaSchema { schema: 0 })
        );
    }

    // FM-10: the entry table of every Container lists at least one Entry, so a
    // meta section with an empty table is rejected on decode.
    #[test]
    fn empty_entry_table_is_rejected() {
        let empty = Meta {
            kind: ContainerKind::Pack,
            pad_len: 0,
            entries: Vec::new(),
        };
        assert_eq!(
            decode(&encode(&empty).expect("encoding succeeds")),
            Err(Error::EmptyEntryTable)
        );
    }

    #[test]
    fn entry_table_with_a_gap_is_rejected() {
        let gapped = Meta {
            kind: ContainerKind::Pack,
            pad_len: 0,
            entries: vec![entry("a.txt", 0, 4), entry("b.txt", 5, 9)],
        };
        assert_eq!(
            decode(&encode(&gapped).expect("encoding succeeds")),
            Err(Error::EntryTableNotContiguous { index: 1 })
        );
    }
}
