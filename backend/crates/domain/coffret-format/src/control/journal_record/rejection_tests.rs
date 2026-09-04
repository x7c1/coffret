//! Journal record payloads a reader refuses (FM-15).

use ciborium::Value;
use coffret_model::Generation;

use super::testing::{first_record, record, GENERATION};
use super::{decode, encode};
use crate::control::testing::{array, body_map, field, with_body_map};
use crate::error::Error;
use crate::ControlPayload;

/// A record payload with one field changed by hand, as a reader meets it.
fn tampered(change: impl FnOnce(&mut Vec<(Value, Value)>)) -> ControlPayload {
    let payload = encode(&record()).expect("encoding succeeds");
    let mut fields = body_map(&payload);
    change(&mut fields);
    with_body_map(payload.master_key_epoch, fields)
}

fn read(payload: &ControlPayload) -> crate::Result<coffret_model::JournalRecord> {
    decode(payload, Generation::new(GENERATION))
}

// FM-15: `additions` is in Container ID order so that one Library state has one
// encoding, and a payload that is not in it is refused rather than sorted.
#[test]
fn additions_out_of_container_id_order_are_rejected() {
    let payload = tampered(|fields| array(fields, "additions").reverse());
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ControlPayloadOutOfOrder {
                array: "additions",
                index: 1
            })
        ),
        "expected reversed additions to be refused, got {result:?}"
    );
}

// FM-15: the same holds for the removals.
#[test]
fn removals_out_of_container_id_order_are_rejected() {
    let payload = tampered(|fields| array(fields, "removals").reverse());
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ControlPayloadOutOfOrder {
                array: "removals",
                index: 1
            })
        ),
        "expected reversed removals to be refused, got {result:?}"
    );
}

// One Container is added once, so a record naming one twice is not a record in
// order with a repeat in it — the same check catches both.
#[test]
fn a_container_added_twice_is_rejected() {
    let payload = tampered(|fields| {
        let additions = array(fields, "additions");
        additions[1] = additions[0].clone();
    });
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ControlPayloadOutOfOrder {
                array: "additions",
                ..
            })
        ),
        "expected a repeated Container to be refused, got {result:?}"
    );
}

// FM-15: `prev` is the record's own statement of the head it was built on, so
// a record at generation g states g - 1 and nothing else. A reader that took
// the object's name for the chain would replay a record at a position its
// authenticated payload never claimed.
#[test]
fn a_prev_that_is_not_the_previous_generation_is_rejected() {
    let payload = tampered(|fields| *field(fields, "prev") = Value::from(GENERATION - 3));
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::JournalRecordPrevMismatch { generation, prev })
                if generation == Generation::new(GENERATION)
                    && prev == Some(Generation::new(GENERATION - 3))
        ),
        "expected a prev naming another head to be refused, got {result:?}"
    );
}

// FM-15: only the record at generation 0 was built on nothing, so a later one
// carrying no `prev` states no head at all.
#[test]
fn a_record_above_generation_zero_without_prev_is_rejected() {
    let payload = tampered(|fields| fields.retain(|(key, _)| key.as_text() != Some("prev")));
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::JournalRecordPrevMismatch {
                generation,
                prev: None
            }) if generation == Generation::new(GENERATION)
        ),
        "expected a record without prev to be refused, got {result:?}"
    );
}

// FM-13: the Library's first head succeeds nothing, so a `prev` on it names a
// head that never existed.
#[test]
fn a_prev_on_the_first_record_is_rejected() {
    let payload = encode(&first_record()).expect("encoding succeeds");
    let mut fields = body_map(&payload);
    fields.push((Value::Text("prev".to_owned()), Value::from(0u64)));
    let payload = with_body_map(payload.master_key_epoch, fields);
    let result = decode(&payload, Generation::FIRST);
    assert!(
        matches!(
            result,
            Err(Error::JournalRecordPrevMismatch {
                generation,
                prev: Some(_)
            }) if generation == Generation::FIRST
        ),
        "expected a prev on the first record to be refused, got {result:?}"
    );
}

// FM-9's rule for the meta section, applied here: a reader accepts any schema
// of 1 or above and refuses anything lower, since a lower one is a form this
// build never learned.
#[test]
fn a_schema_below_one_is_rejected() {
    let payload = tampered(|fields| *field(fields, "schema") = Value::from(0u64));
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::UnsupportedJournalRecordSchema { schema: 0 })
        ),
        "expected schema 0 to be unreadable, got {result:?}"
    );
}

#[test]
fn a_missing_field_is_reported_by_name() {
    let payload = tampered(|fields| fields.retain(|(key, _)| key.as_text() != Some("removals")));
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { ref detail }) if detail.contains("removals")),
        "expected the missing field to be named, got {result:?}"
    );
}

#[test]
fn a_field_of_the_wrong_shape_is_reported_by_name() {
    let payload = tampered(|fields| {
        *field(fields, "keyring_set_digest") = Value::from(9u64);
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { ref detail })
            if detail.contains("keyring_set_digest")),
        "expected the field to be named, got {result:?}"
    );
}

// FM-12, KL-3: the digest is the lowercase hex token a Keyring replica's name
// carries, so a payload spelling it otherwise names no replica set this build
// can select against.
#[test]
fn a_keyring_digest_that_is_not_lowercase_hex_is_rejected() {
    let payload = tampered(|fields| {
        *field(fields, "keyring_set_digest") = Value::Text("BEEF".to_owned());
    });
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::Model(coffret_model::Error::InvalidSetDigest { ref digest })) if digest == "BEEF"
        ),
        "expected an uppercase digest to be refused, got {result:?}"
    );
}

// FM-15: a Container ID is sixteen bytes, and a removal that is not is a value
// no Container ever had.
#[test]
fn a_removal_of_the_wrong_length_is_rejected() {
    let payload = tampered(|fields| {
        array(fields, "removals")[0] = Value::Bytes(vec![0x11; 4]);
    });
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::Model(coffret_model::Error::InvalidByteLength {
                expected: 16,
                actual: 4
            }))
        ),
        "expected a four-byte removal to be refused, got {result:?}"
    );
}

#[test]
fn a_removal_that_is_not_a_byte_string_is_rejected() {
    let payload = tampered(|fields| {
        array(fields, "removals")[0] = Value::Text("11111111".to_owned());
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { ref detail }) if detail.contains("removal")),
        "expected a text removal to be refused, got {result:?}"
    );
}

// FM-15: an addition carries an entry table, and each element of it is one
// entry map — the same values FM-9's is, under the catalog's own keys — so an
// element that is not a map at all is refused here as it is there.
#[test]
fn an_entry_that_is_not_an_entry_map_is_rejected() {
    let payload = tampered(|fields| {
        let Value::Map(addition) = &mut array(fields, "additions")[0] else {
            panic!("an addition is a map");
        };
        let Value::Array(entries) = field(addition, "entries") else {
            panic!("an entry table is an array");
        };
        entries[0] = Value::Text("not an entry".to_owned());
    });
    assert!(matches!(
        read(&payload),
        Err(Error::MalformedJournalRecord { .. })
    ));
}

// PK-15: `kind` names one of the two kinds a Container can be, so a spelling
// this format version has no kind for is refused rather than guessed at.
#[test]
fn an_addition_of_an_unknown_kind_is_rejected() {
    let payload = tampered(|fields| {
        let Value::Map(addition) = &mut array(fields, "additions")[0] else {
            panic!("an addition is a map");
        };
        *field(addition, "kind") = Value::Text("archive".to_owned());
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { ref detail }) if detail.contains("archive")),
        "expected an unknown kind to be refused, got {result:?}"
    );
}

// FM-11 takes the padding off before this reader sees a body, so bytes after
// the map are bytes no writer following the rule left there.
#[test]
fn bytes_after_the_body_map_are_rejected() {
    let mut payload = encode(&record()).expect("encoding succeeds");
    payload.body.push(0x00);
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { ref detail }) if detail.contains("follow")),
        "expected a trailing byte to be refused, got {result:?}"
    );
}

#[test]
fn a_body_that_is_not_a_map_is_rejected() {
    let payload = encode(&record()).expect("encoding succeeds");
    let mut body = Vec::new();
    ciborium::into_writer(&Value::Text("not a map".to_owned()), &mut body)
        .expect("text serializes");
    let result = read(&ControlPayload::new(payload.master_key_epoch, body));
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { .. })),
        "expected a body that is not a map to be refused, got {result:?}"
    );
}

// EP-2: an entry table in a record carries paths the Library already holds, so
// one outside the shape every Entry Path is in was written by something that did
// not hold to EP-2 — the record does not decode, and the field that carried it
// is named.
#[test]
fn an_entry_path_with_a_shape_ep_2_excludes_is_rejected() {
    let payload = tampered(|fields| {
        let Value::Map(addition) = &mut array(fields, "additions")[0] else {
            panic!("an addition is a CBOR map");
        };
        let Value::Array(entries) = field(addition, "entries") else {
            panic!("an entry table is a CBOR array");
        };
        let Value::Map(entry) = &mut entries[0] else {
            panic!("an entry is a CBOR map");
        };
        *field(entry, "path") = Value::Text("../x".to_owned());
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedEntryPath { field: "path" })),
        "expected a `..` component to be refused, got {result:?}"
    );
}

// FM-9, FM-15: an addition carries the entry table of the Container it adds, and
// each row of it places an Entry against a plaintext stream addressed in 64
// bits. A row whose `offset` and `size` end past that address space places
// nothing, so the record does not decode — the same refusal a meta section
// carrying such a row gets, because it is the same table.
#[test]
fn an_entry_extent_past_the_end_of_the_address_space_is_rejected() {
    let payload = tampered(|fields| {
        let Value::Map(addition) = &mut array(fields, "additions")[0] else {
            panic!("an addition is a CBOR map");
        };
        let Value::Array(entries) = field(addition, "entries") else {
            panic!("an entry table is a CBOR array");
        };
        let Value::Map(entry) = &mut entries[0] else {
            panic!("an entry is a CBOR map");
        };
        *field(entry, "offset") = Value::from(u64::MAX);
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::StreamTooLong)),
        "expected an extent running past the address space to be refused, got {result:?}"
    );
}

// FM-10: a Container is built out of Entries, so an addition whose table holds
// none describes a Container no writer produces — and a record carrying one
// would add a Container to every Index without adding a single Entry. The
// meta-section reader has always refused it; the record's reader refuses it
// now, because the rule belongs to the addition rather than to either reader.
#[test]
fn an_addition_with_no_entries_is_rejected() {
    let payload = tampered(|fields| {
        let Value::Map(addition) = &mut array(fields, "additions")[0] else {
            panic!("an addition is a CBOR map");
        };
        *field(addition, "entries") = Value::Array(Vec::new());
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::AdditionWithoutEntries { addition: 0 })),
        "expected an addition with an empty table to be refused, got {result:?}"
    );
}

// FM-9: the entry table tiles the Container's plaintext stream, so every Entry
// begins where its predecessor ended. A record whose table leaves a gap would
// apply to an Index that then answers a range read with bytes belonging to
// nothing.
#[test]
fn an_addition_whose_entries_do_not_tile_is_rejected() {
    let payload = tampered(|fields| {
        // The addition at 1 is the Pack, whose table carries two Entries.
        let Value::Map(addition) = &mut array(fields, "additions")[1] else {
            panic!("an addition is a CBOR map");
        };
        let Value::Array(entries) = field(addition, "entries") else {
            panic!("an entry table is a CBOR array");
        };
        let Value::Map(entry) = &mut entries[1] else {
            panic!("an entry is a CBOR map");
        };
        *field(entry, "offset") = Value::from(130u64);
    });
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::AdditionEntriesDoNotTile {
                addition: 1,
                entry: 1,
                expected: 120,
                found: 130,
            })
        ),
        "expected a gap in an entry table to be refused, got {result:?}"
    );
}
