use super::wire_entry::WireEntry;
use super::wire_meta::WireMeta;
use super::Meta;
use super::SCHEMA;
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
    let wire: WireMeta =
        ciborium::from_reader(&mut padding).map_err(|error| Error::MalformedMeta {
            detail: error.to_string(),
        })?;

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
    use super::super::testing::{as_value, entry, padded, sample, to_bytes};
    use super::*;

    /// The sample's plaintext, and where its CBOR map ends inside it.
    fn sample_plaintext() -> (Vec<u8>, usize) {
        let map = encode(&sample()).expect("encoding succeeds");
        let map_len = map.len();
        let plaintext = padded(map);
        assert!(
            map_len < plaintext.len(),
            "this meta section carries no padding to check"
        );
        (plaintext, map_len)
    }

    // FM-9: the meta section's plaintext is the CBOR map followed by zero
    // padding to the map's Padmé bucket, and CBOR is self-delimiting, so a
    // reader takes one item and holds the rest to that rule.
    #[test]
    fn a_map_padded_to_its_bucket_is_accepted() {
        let (plaintext, _) = sample_plaintext();
        assert_eq!(
            decode(&plaintext)
                .expect("a padded map is accepted")
                .entries,
            sample().entries
        );
    }

    // FM-9: any non-zero byte after the CBOR map fails decode, so the padding is
    // not a place to smuggle bytes past a reader.
    #[test]
    fn a_non_zero_byte_after_the_map_is_rejected() {
        let (plaintext, map_len) = sample_plaintext();
        for index in map_len..plaintext.len() {
            let mut tampered = plaintext.clone();
            tampered[index] = 0x01;
            let result = decode(&tampered);
            assert!(
                matches!(result, Err(Error::NonZeroMetaPadding)),
                "byte {index} of the padding was not checked, got {result:?}"
            );
        }
    }

    // FM-9: the plaintext is the map and its padding and nothing else, so a
    // zero byte beyond the bucket is a length no writer following the rule
    // produces — and it would put the header's meta section length past what
    // the map accounts for (FM-2).
    #[test]
    fn a_plaintext_longer_than_the_bucket_is_rejected() {
        let (mut plaintext, _) = sample_plaintext();
        let bucket = plaintext.len() as u64;
        plaintext.push(0x00);
        let result = decode(&plaintext);
        assert!(
            matches!(
                result,
                Err(Error::MetaPaddingLengthMismatch { expected, actual })
                    if expected == bucket && actual == bucket + 1
            ),
            "expected a plaintext past the bucket to be rejected, got {result:?}"
        );
    }

    // FM-9: a writer that skipped the padding leaks the size the padding exists
    // to blur, so its object is refused rather than quietly read.
    #[test]
    fn an_unpadded_map_is_rejected() {
        let (plaintext, map_len) = sample_plaintext();
        let result = decode(&plaintext[..map_len]);
        assert!(
            matches!(
                result,
                Err(Error::MetaPaddingLengthMismatch { expected, actual })
                    if expected == plaintext.len() as u64 && actual == map_len as u64
            ),
            "expected an unpadded map to be rejected, got {result:?}"
        );
    }

    #[test]
    fn optional_fields_round_trip() {
        let mut meta = sample();
        meta.entries[0].mime = Some("text/plain".to_owned());
        meta.entries[0].derived_from = Some(DerivedFrom {
            container_id: ContainerId::from_bytes([3u8; ContainerId::BYTE_LEN]),
            path: EntryPath::new("originals/a.txt"),
        });
        let plaintext = padded(encode(&meta).expect("encoding succeeds"));
        let decoded = decode(&plaintext).expect("decoding succeeds");
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
        let result = decode(&to_bytes(&Value::Map(map)));
        assert!(
            matches!(result, Err(Error::UnsupportedMetaSchema { schema: 0 })),
            "expected schema 0 to be unreadable, got {result:?}"
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
        let result = decode(&padded(encode(&empty).expect("encoding succeeds")));
        assert!(
            matches!(result, Err(Error::EmptyEntryTable)),
            "expected an empty entry table to be refused, got {result:?}"
        );
    }

    #[test]
    fn entry_table_with_a_gap_is_rejected() {
        let gapped = Meta {
            kind: ContainerKind::Pack,
            pad_len: 0,
            entries: vec![entry("a.txt", 0, 4), entry("b.txt", 5, 9)],
        };
        let result = decode(&padded(encode(&gapped).expect("encoding succeeds")));
        assert!(
            matches!(result, Err(Error::EntryTableNotContiguous { index: 1 })),
            "expected the gap before entry 1 to be refused, got {result:?}"
        );
    }
}
