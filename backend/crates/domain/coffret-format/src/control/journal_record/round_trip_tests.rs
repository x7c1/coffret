//! What survives a trip through a Journal record payload and back (FM-15).

use ciborium::Value;
use coffret_model::{ContainerKind, Generation};

use super::testing::{addition, first_record, record, GENERATION};
use super::{decode, encode};
use crate::control::testing::container_id;

// FM-15: a record's fields come back as they went in — the Keyring tuple it
// commits to, both slots it reserves, the Containers it added with their entry
// tables, and the ones it removed.
#[test]
fn a_record_with_everything_round_trips() {
    let record = record();
    let payload = encode(&record).expect("encoding succeeds");
    let decoded = decode(&payload, Generation::new(GENERATION)).expect("the payload reads back");
    assert_eq!(decoded, canonical(record));
}

// FM-15, CP-2, CP-15: at generation 0 there is no predecessor, and a name-keyed
// Storage persists no slot token at all, so all three optional fields are
// absent — and absent is not the same as present-and-empty on the way back.
#[test]
fn a_record_with_no_predecessor_and_no_minted_slots_round_trips() {
    let record = first_record();
    let payload = encode(&record).expect("encoding succeeds");
    let decoded = decode(&payload, Generation::FIRST).expect("the payload reads back");
    assert_eq!(decoded, canonical(record));
    assert_eq!(decoded.prev, None);
    assert_eq!(decoded.next_commit_slot, None);
    assert_eq!(decoded.snapshot_slot, None);
}

// FM-15: the three optional fields are left out of the map rather than written
// as something empty, so a reader never has two spellings of "nothing" to tell
// apart.
#[test]
fn absent_optional_fields_are_not_written_at_all() {
    let payload = encode(&first_record()).expect("encoding succeeds");
    for absent in ["prev", "next_commit_slot", "snapshot_slot"] {
        assert!(
            !keys(&payload.body).contains(&absent.to_owned()),
            "{absent} was written for a record that carries none"
        );
    }
}

// FM-15: the arrays are in Container ID order, whatever order the record was
// held in, so one Library state has exactly one encoding. Two records with the
// same content are byte-for-byte the same payload.
#[test]
fn the_same_content_in_a_different_order_encodes_identically() {
    let mut reordered = record();
    reordered.additions.reverse();
    reordered.removals.reverse();

    let one = encode(&record()).expect("encoding succeeds");
    let other = encode(&reordered).expect("encoding succeeds");
    assert_eq!(one.body, other.body);
}

// FM-13: the record's own generation is the header's, and the epoch is the
// payload field every kind carries — so neither is repeated in the map, and
// what comes back is what the framing was told.
#[test]
fn the_generation_and_the_epoch_come_from_the_framing() {
    let payload = encode(&record()).expect("encoding succeeds");
    let written = keys(&payload.body);
    assert!(!written.contains(&"generation".to_owned()), "{written:?}");
    assert!(
        !written.contains(&"master_key_epoch".to_owned()),
        "{written:?}"
    );

    let decoded = decode(&payload, Generation::new(GENERATION)).expect("the payload reads back");
    assert_eq!(decoded.generation, Generation::new(GENERATION));
    assert_eq!(decoded.master_key_epoch, payload.master_key_epoch);
}

// FM-15: `additions` and `removals` may both be empty — a commit that only
// removes Containers adds none, and one that only adds removes none.
#[test]
fn a_record_that_only_removes_round_trips() {
    let mut record = record();
    record.additions.clear();
    let payload = encode(&record).expect("encoding succeeds");
    let decoded = decode(&payload, Generation::new(GENERATION)).expect("the payload reads back");
    assert!(decoded.additions.is_empty());
    assert_eq!(decoded.removals.len(), 2);
}

// FM-9: the maps are forward-open, so a field a newer writer added is stepped
// over rather than refused — at the record's own level and inside an addition.
#[test]
fn unknown_fields_are_ignored() {
    use crate::control::testing::{array, body_map, with_body_map};

    let payload = encode(&record()).expect("encoding succeeds");
    let mut fields = body_map(&payload);
    fields.push((
        Value::Text("future_field".to_owned()),
        Value::Text("whatever".to_owned()),
    ));
    for addition in array(&mut fields, "additions") {
        let Value::Map(map) = addition else {
            panic!("an addition is a map");
        };
        map.push((
            Value::Text("future_addition_field".to_owned()),
            Value::from(1u64),
        ));
    }
    // A newer writer would also have bumped `schema`.
    *crate::control::testing::field(&mut fields, "schema") = Value::from(2u64);

    let extended = with_body_map(payload.master_key_epoch, fields);
    let decoded =
        decode(&extended, Generation::new(GENERATION)).expect("unknown fields are ignored");
    assert_eq!(decoded, canonical(record()));
}

// CP-14: a removal is the Container ID and nothing else, so a removed Container
// costs sixteen bytes in a record however much it held.
#[test]
fn a_removal_is_the_container_id_alone() {
    let payload = encode(&record()).expect("encoding succeeds");
    let mut fields = crate::control::testing::body_map(&payload);
    let removals = crate::control::testing::array(&mut fields, "removals");
    assert_eq!(
        removals,
        &vec![
            Value::Bytes(container_id(0x11).as_bytes().to_vec()),
            Value::Bytes(container_id(0x99).as_bytes().to_vec()),
        ]
    );
}

// PK-15: the kind is the explicit one the Container recorded, not one guessed
// from how many Entries the addition carries.
#[test]
fn a_singleton_pack_stays_a_pack() {
    let mut record = first_record();
    record.additions = vec![addition(0x30, ContainerKind::OneFile)];
    record.additions[0].container.kind = ContainerKind::Pack;

    let payload = encode(&record).expect("encoding succeeds");
    let decoded = decode(&payload, Generation::FIRST).expect("the payload reads back");
    assert_eq!(decoded.additions[0].entries.len(), 1);
    assert_eq!(decoded.additions[0].container.kind, ContainerKind::Pack);
}

/// The record as the encoder puts it on the wire: arrays in Container ID order.
fn canonical(mut record: coffret_model::JournalRecord) -> coffret_model::JournalRecord {
    record
        .additions
        .sort_by_key(|addition| addition.container.id);
    record.removals.sort();
    record
}

/// The field names a payload body carries, in the order the encoder wrote them.
fn keys(body: &[u8]) -> Vec<String> {
    let value: Value = ciborium::from_reader(body).expect("a payload body is CBOR");
    let Value::Map(fields) = value else {
        panic!("a payload body is a map");
    };
    fields
        .iter()
        .map(|(key, _)| key.as_text().expect("keys are text").to_owned())
        .collect()
}
