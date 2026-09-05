//! What a meta section is refused for rather than quietly read (FM-9, FM-10,
//! EP-1, EP-2).

use ciborium::Value;
use coffret_model::{ContainerId, ContainerKind, DerivedFrom, MAX_FORMAT_INTEGER};

use super::testing::{as_value, entry, padded, sample, sample_plaintext, to_bytes};
use super::{decode, encode, Meta};
use crate::entry_paths::entry_path;
use crate::error::Error;

/// `café.txt` with the accent as `e` and a combining acute — a spelling no
/// writer holding to EP-1 ever puts in an entry table.
const DECOMPOSED: &str = "cafe\u{301}.txt";

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

// FM-9: the plaintext is the map and its padding and nothing else, so a zero
// byte beyond the bucket is a length no writer following the rule produces —
// and it would put the header's meta section length past what the map accounts
// for (FM-2).
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

// FM-9: a writer that skipped the padding leaks the size the padding exists to
// blur, so its object is refused rather than quietly read.
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

// FM-9: `original_path` is where an Entry's position is recorded, so an entry
// map without one describes nothing. It is refused exactly as a map without the
// key this rule used to spell `path` was.
#[test]
fn an_entry_map_without_an_original_path_is_rejected() {
    let plaintext = tampered_entry(&sample(), |fields| {
        fields.retain(|(key, _)| key.as_text() != Some("original_path"));
    });
    let result = decode(&plaintext);
    let Err(Error::MalformedMeta { detail }) = result else {
        panic!("expected an entry map without its Entry Path to be refused, got {result:?}");
    };
    // What the deserializer said, not ciborium's `Debug` spelling of it: a
    // detail reading `Semantic(None, "…")` would name the layer that caught the
    // map rather than the field it found missing.
    assert!(detail.contains("original_path"), "{detail}");
    assert!(!detail.contains("Semantic"), "{detail}");
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

// FM-10: the entry table of every Container lists at least one Entry, so a meta
// section with an empty table is rejected on decode.
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

// EP-1: the paths in a meta section are ones the Library already holds, so a
// decomposed one is a malformed payload and the object is refused rather than
// composed on the way back in.
#[test]
fn an_entry_path_that_is_not_in_nfc_is_rejected() {
    let plaintext = tampered_entry(&sample(), |fields| {
        *field(fields, "original_path") = Value::Text(DECOMPOSED.to_owned());
    });
    let result = decode(&plaintext);
    assert!(
        matches!(
            result,
            Err(Error::UnnormalizedEntryPath {
                field: "original_path"
            })
        ),
        "expected a decomposed Entry Path to be refused, got {result:?}"
    );
}

// The same rule reaches the path inside a `derived_from` reference, which names
// an Entry of the Library just as much as the entry's own path does.
#[test]
fn a_derived_from_path_that_is_not_in_nfc_is_rejected() {
    let plaintext = tampered_entry(&sample_with_derived_from(), |fields| {
        let Value::Map(derived) = field(fields, "derived_from") else {
            panic!("derived_from is a CBOR map");
        };
        *field(derived, "original_path") = Value::Text(DECOMPOSED.to_owned());
    });
    let result = decode(&plaintext);
    assert!(
        matches!(
            result,
            Err(Error::UnnormalizedEntryPath {
                field: "derived_from.original_path"
            })
        ),
        "expected a decomposed derived-from path to be refused, got {result:?}"
    );
}

// EP-2: the paths in a meta section are ones the Library already holds, so one
// outside the shape every Entry Path is in was written by something that did not
// hold to EP-2 — the Container does not open, the same way it does not open for
// a path that is not NFC.
#[test]
fn an_entry_path_with_a_shape_ep_2_excludes_is_rejected() {
    let plaintext = tampered_entry(&sample(), |fields| {
        *field(fields, "original_path") = Value::Text("../x".to_owned());
    });
    let result = decode(&plaintext);
    assert!(
        matches!(
            result,
            Err(Error::MalformedEntryPath {
                field: "original_path"
            })
        ),
        "expected a `..` component to be refused, got {result:?}"
    );
}

// The same rule reaches the path inside a `derived_from` reference, which names
// an Entry of the Library just as much as the entry's own path does.
#[test]
fn a_derived_from_path_with_a_shape_ep_2_excludes_is_rejected() {
    let plaintext = tampered_entry(&sample_with_derived_from(), |fields| {
        let Value::Map(derived) = field(fields, "derived_from") else {
            panic!("derived_from is a CBOR map");
        };
        *field(derived, "original_path") = Value::Text("albums/spring.jpg/".to_owned());
    });
    let result = decode(&plaintext);
    assert!(
        matches!(
            result,
            Err(Error::MalformedEntryPath {
                field: "derived_from.original_path"
            })
        ),
        "expected a trailing separator to be refused, got {result:?}"
    );
}

// FM-9, FM-19: an Entry's `offset` and `size` describe a range of a plaintext
// stream whose positions the format bounds, so a pair whose end lies past the
// last of them places no Entry at all. The verdict is the one a table whose
// entries together outrun the stream already gets, so one Container yields one
// error whichever of the two checks meets it first.
#[test]
fn an_entry_extent_past_the_end_of_the_address_space_is_rejected() {
    let plaintext = tampered_entry(&sample(), |fields| {
        *field(fields, "offset") = Value::from(MAX_FORMAT_INTEGER);
    });
    let result = decode(&plaintext);
    assert!(
        matches!(result, Err(Error::StreamTooLong)),
        "expected an extent running past the address space to be refused, got {result:?}"
    );
}

// FM-19: every unsigned integer a meta section carries is below 2^63, `schema`
// among them — and the schema check refuses only a version older than this
// build's, so a number past the bound would otherwise be read on as a newer
// one. The two reader cases below carry the section's other integers, and what
// the detail of such a refusal says.
#[test]
fn a_meta_integer_past_the_formats_integer_range_is_malformed() {
    let Value::Map(mut map) = as_value(&sample()) else {
        panic!("the meta section is a CBOR map");
    };
    *field(&mut map, "schema") = Value::from(MAX_FORMAT_INTEGER + 1);
    let result = decode(&to_bytes(&Value::Map(map)));
    assert!(
        matches!(result, Err(Error::MalformedMeta { .. })),
        "expected a schema of 2^63 to be malformed, got {result:?}"
    );
}

// FM-19: the refusal names the field and the number, the way a control
// payload's already does — both are the format's own arithmetic and neither
// says anything about the Library's content. The map is deserialized as a whole
// struct, so what a caller must not see is the serializer's account of it:
// ciborium spells its own errors in `Debug`, and a detail reading
// `Semantic(None, "…")` would name the layer that caught the number rather than
// the field that carried it.
#[test]
fn a_meta_integer_past_the_formats_integer_range_names_its_field() {
    let Value::Map(mut map) = as_value(&sample()) else {
        panic!("the meta section is a CBOR map");
    };
    let past_the_bound = MAX_FORMAT_INTEGER + 1;
    *field(&mut map, "pad_len") = Value::from(past_the_bound);

    let result = decode(&to_bytes(&Value::Map(map)));
    let Err(Error::MalformedMeta { detail }) = result else {
        panic!("expected a pad_len of 2^63 to be malformed, got {result:?}");
    };
    assert!(detail.contains("pad_len"), "{detail}");
    assert!(detail.contains("below 2^63"), "{detail}");
    assert!(detail.contains(&past_the_bound.to_string()), "{detail}");
    assert!(!detail.contains("Semantic"), "{detail}");
}

// FM-9, FM-19: an entry's own numbers are named the same way, out of the same
// map read as one struct.
#[test]
fn an_entry_integer_past_the_formats_integer_range_names_its_field() {
    let past_the_bound = MAX_FORMAT_INTEGER + 1;
    let plaintext = tampered_entry(&sample(), |fields| {
        *field(fields, "size") = Value::from(past_the_bound);
    });

    let result = decode(&plaintext);
    let Err(Error::MalformedMeta { detail }) = result else {
        panic!("expected a size of 2^63 to be malformed, got {result:?}");
    };
    assert!(detail.contains("size"), "{detail}");
    assert!(detail.contains(&past_the_bound.to_string()), "{detail}");
}

// FM-19: a writer never produces a number past the bound either, so a section
// written here is never one this crate's own reader — or the TypeScript
// implementation's — refuses. The padding length is the only number the encoder
// has to say that about, for the reason `bounded_uint` gives.
#[test]
fn a_pad_len_past_the_formats_integer_range_is_not_written() {
    let mut meta = sample();
    meta.pad_len = MAX_FORMAT_INTEGER + 1;

    let result = encode(&meta);
    let Err(Error::MetaEncodeFailed { detail }) = result else {
        panic!("expected a pad_len of 2^63 to be refused, got {result:?}");
    };
    assert!(detail.contains("pad_len"), "{detail}");
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

/// The sample with a `derived_from` reference on its first entry.
fn sample_with_derived_from() -> Meta {
    let mut meta = sample();
    meta.entries[0].derived_from = Some(DerivedFrom {
        container_id: ContainerId::from_bytes([3u8; ContainerId::BYTE_LEN]),
        path: entry_path("originals/a.txt"),
    });
    meta
}

/// `meta` as the plaintext a writer would produce, with `edit` applied to the
/// CBOR map of its first entry.
///
/// A value the domain refuses cannot be built through `EntryMetadata`, which is
/// the point of the invariant, so a payload carrying one is written by hand at
/// the wire shape — which is also how it would arrive.
fn tampered_entry(meta: &Meta, edit: impl FnOnce(&mut Vec<(Value, Value)>)) -> Vec<u8> {
    let Value::Map(mut map) = as_value(meta) else {
        panic!("the meta section is a CBOR map");
    };
    let entries = field(&mut map, "entries");
    let Value::Array(entries) = entries else {
        panic!("entries is an array");
    };
    let Some(Value::Map(fields)) = entries.first_mut() else {
        panic!("an entry is a CBOR map");
    };
    edit(fields);
    to_bytes(&Value::Map(map))
}

/// The value one key of a CBOR map holds.
fn field<'a>(map: &'a mut [(Value, Value)], key: &str) -> &'a mut Value {
    map.iter_mut()
        .find(|(name, _)| name.as_text() == Some(key))
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("the map carries {key}"))
}
