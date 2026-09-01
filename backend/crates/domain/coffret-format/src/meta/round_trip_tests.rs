//! What survives a trip through a meta section and back, and the spelling the
//! encoder writes it in (FM-9).

use ciborium::Value;
use coffret_model::{Btime, ContainerId, ContainerKind, DerivedFrom, EntryPath};

use super::testing::{as_value, padded, sample, sample_plaintext, to_bytes};
use super::{decode, encode};

// FM-9: the meta section's plaintext is the CBOR map followed by zero padding
// to the map's Padmé bucket, and CBOR is self-delimiting, so a reader takes one
// item and holds the rest to that rule.
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

#[test]
fn optional_fields_round_trip() {
    let mut meta = sample();
    meta.entries[0].mime = Some("text/plain".to_owned());
    meta.entries[0].derived_from = Some(DerivedFrom {
        container_id: ContainerId::from_bytes([3u8; ContainerId::BYTE_LEN]),
        path: EntryPath::nfc("originals/a.txt"),
    });
    let plaintext = padded(encode(&meta).expect("encoding succeeds"));
    let decoded = decode(&plaintext).expect("decoding succeeds");
    assert_eq!(decoded.entries, meta.entries);
}

// FM-9: `original_btime` is optional, and the two answers are "this is when the
// file was created" and "no birth time was ever captured" — never a stand-in
// value. One table carries both, so a round trip that quietly filled the absent
// one in would show up here.
#[test]
fn a_birth_time_round_trips_where_one_was_captured() {
    let mut meta = sample();
    meta.entries[0].btime = Some(Btime::from_unix_seconds(-86_400));
    let decoded =
        decode(&padded(encode(&meta).expect("encoding succeeds"))).expect("decoding succeeds");
    assert_eq!(
        decoded.entries[0].btime,
        Some(Btime::from_unix_seconds(-86_400)),
        "a birth time before 1970 is preserved rather than corrected",
    );
    assert_eq!(
        decoded.entries[1].btime, None,
        "and an entry written without one still has none",
    );
}

// FM-9: the maps are forward-open — a reader ignores fields it does not know,
// so a newer writer can add fields without breaking this reader.
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

// FM-9: the meta section is one CBOR map with `schema`, `kind`, `pad_len`, and
// `entries`; each entry records `original_path`, `offset`, `size`,
// `original_mtime`, and `hash`, plus optional `original_btime`, `derived_from`,
// and `mime`.
#[test]
fn field_names_and_kind_spelling_match_the_rule() {
    let value = as_value(&sample());
    let map = value.as_map().expect("the meta section is a CBOR map");
    let keys: Vec<&str> = map
        .iter()
        .map(|(key, _)| key.as_text().expect("keys are text"))
        .collect();
    assert_eq!(keys, ["schema", "kind", "pad_len", "entries"]);

    assert_eq!(container_kind(&value), Some("pack"));
    assert_eq!(
        entry_keys(&entries(&value)[0]),
        ["original_path", "offset", "size", "original_mtime", "hash"]
    );
}

// FM-9: the birth time is optional, so a Container written where the filesystem
// reported one carries the field and one written where it did not carries no
// key at all — absent means "never captured" rather than "created at the
// epoch".
#[test]
fn a_birth_time_is_written_beside_the_modification_time() {
    let mut meta = sample();
    meta.entries[0].btime = Some(Btime::from_unix_seconds(-86_400));
    let value = as_value(&meta);
    let entries = entries(&value);

    assert_eq!(
        entry_keys(&entries[0]),
        [
            "original_path",
            "offset",
            "size",
            "original_mtime",
            "original_btime",
            "hash"
        ],
    );
    assert_eq!(
        entry_keys(&entries[1]),
        ["original_path", "offset", "size", "original_mtime", "hash"],
        "the entry whose file had no birth time carries no key for one",
    );
}

// FM-9: the other spelling `kind` carries is `one-file`.
#[test]
fn one_file_kind_spelling_matches_the_rule() {
    let mut meta = sample();
    meta.kind = ContainerKind::OneFile;
    assert_eq!(container_kind(&as_value(&meta)), Some("one-file"));
}

/// The Container kind one meta section map spells, as it was written.
fn container_kind(meta: &Value) -> Option<&str> {
    field(meta, "kind").as_text()
}

/// The entry table one meta section map carries, as it was written.
fn entries(meta: &Value) -> Vec<Value> {
    field(meta, "entries")
        .as_array()
        .expect("entries is an array")
        .clone()
}

/// The value one key of the meta section map holds.
fn field<'a>(meta: &'a Value, key: &str) -> &'a Value {
    meta.as_map()
        .expect("the meta section is a CBOR map")
        .iter()
        .find(|(name, _)| name.as_text() == Some(key))
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("the map carries {key}"))
}

/// The keys of one entry map, in the order they were written.
fn entry_keys(entry: &Value) -> Vec<&str> {
    entry
        .as_map()
        .expect("an entry is a CBOR map")
        .iter()
        .map(|(key, _)| key.as_text().expect("keys are text"))
        .collect()
}
