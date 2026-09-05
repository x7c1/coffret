//! What survives a trip through a Journal record payload and back (FM-15).

use ciborium::Value;
use coffret_model::{ContainerKind, Generation};

use super::testing::{
    addition, addition_of, first_record, first_record_of, record, record_of, table, BORN,
    GENERATION,
};
use super::{decode, encode};
use crate::control::testing::{
    array, body_keys, body_map, container_id, field, map_keys, with_body_map,
};
use crate::control::ControlPayload;
use crate::generations::generation;

// FM-15: a record's fields come back as they went in — the Keyring tuple it
// commits to, both slots it reserves, the Containers it added with their entry
// tables, and the ones it removed.
#[test]
fn a_record_with_everything_round_trips() {
    let record = record();
    let payload = encode(&record).expect("encoding succeeds");
    let decoded = decode(&payload, generation(GENERATION)).expect("the payload reads back");
    assert_eq!(decoded, record);
}

// FM-15, CP-2, CP-15: at generation 0 there is no predecessor, and a name-keyed
// Storage persists no slot token at all, so all three optional fields are
// absent — and absent is not the same as present-and-empty on the way back.
#[test]
fn a_record_with_no_predecessor_and_no_minted_slots_round_trips() {
    let record = first_record();
    let payload = encode(&record).expect("encoding succeeds");
    let decoded = decode(&payload, Generation::FIRST).expect("the payload reads back");
    assert_eq!(decoded, record);
    assert_eq!(decoded.prev(), None);
    assert_eq!(decoded.next_commit_slot(), None);
    assert_eq!(decoded.snapshot_slot(), None);
}

// FM-15: the three optional fields are left out of the map rather than written
// as something empty, so a reader never has two spellings of "nothing" to tell
// apart.
#[test]
fn absent_optional_fields_are_not_written_at_all() {
    let payload = encode(&first_record()).expect("encoding succeeds");
    for absent in ["prev", "next_commit_slot", "snapshot_slot"] {
        assert!(
            !body_keys(&payload).contains(&absent.to_owned()),
            "{absent} was written for a record that carries none"
        );
    }
}

// FM-15: the arrays are in Container ID order whichever order a writer handed
// them over in, so one Library state has exactly one encoding. The record holds
// them in that order, so two records built from the same content are the same
// value and therefore byte-for-byte the same payload.
#[test]
fn the_same_content_in_a_different_order_encodes_identically() {
    let reordered = record_of(
        vec![
            addition(0x21, ContainerKind::OneFile),
            addition(0x40, ContainerKind::Pack),
        ],
        vec![container_id(0x11), container_id(0x99)],
    );
    assert_eq!(reordered, record());

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
    let written = body_keys(&payload);
    assert!(!written.contains(&"generation".to_owned()), "{written:?}");
    assert!(
        !written.contains(&"master_key_epoch".to_owned()),
        "{written:?}"
    );

    let decoded = decode(&payload, generation(GENERATION)).expect("the payload reads back");
    assert_eq!(decoded.generation(), generation(GENERATION));
    assert_eq!(decoded.master_key_epoch(), payload.master_key_epoch);
}

// FM-15: `additions` and `removals` may both be empty — a commit that only
// removes Containers adds none, and one that only adds removes none.
#[test]
fn a_record_that_only_removes_round_trips() {
    let record = record_of(Vec::new(), vec![container_id(0x99), container_id(0x11)]);
    let payload = encode(&record).expect("encoding succeeds");
    let decoded = decode(&payload, generation(GENERATION)).expect("the payload reads back");
    assert!(decoded.additions().is_empty());
    assert_eq!(decoded.removals().len(), 2);
}

// FM-9: the maps are forward-open, so a field a newer writer added is stepped
// over rather than refused — at the record's own level and inside an addition.
#[test]
fn unknown_fields_are_ignored() {
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
    *field(&mut fields, "schema") = Value::from(2u64);

    let extended = with_body_map(payload.master_key_epoch, fields);
    let decoded = decode(&extended, generation(GENERATION)).expect("unknown fields are ignored");
    assert_eq!(decoded, record());
}

// CP-14: a removal is the Container ID and nothing else, so a removed Container
// costs sixteen bytes in a record however much it held.
#[test]
fn a_removal_is_the_container_id_alone() {
    let payload = encode(&record()).expect("encoding succeeds");
    let mut fields = body_map(&payload);
    let removals = array(&mut fields, "removals");
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
    let record = first_record_of(vec![addition_of(
        0x30,
        ContainerKind::Pack,
        table(0x30, ContainerKind::OneFile),
    )]);

    let payload = encode(&record).expect("encoding succeeds");
    let decoded = decode(&payload, Generation::FIRST).expect("the payload reads back");
    assert_eq!(decoded.additions()[0].entries().len(), 1);
    assert_eq!(decoded.additions()[0].container().kind, ContainerKind::Pack);
}

// FM-15: a record's entry table is the catalog's own spelling — `path`,
// `mtime`, and an optional `btime`, without the `original_` prefix FM-9 gives
// the meta section's copy of the same values. An addition listing them under
// FM-9's keys would be a record no replay could read.
#[test]
fn an_entry_table_carries_the_catalog_spelling() {
    let payload = encode(&record()).expect("encoding succeeds");
    let table = entry_table(&payload, PACK);
    assert_eq!(
        map_keys(&table[1]),
        [
            "path",
            "offset",
            "size",
            "mtime",
            "btime",
            "hash",
            "derived_from",
            "mime"
        ],
    );
}

// FM-15: `btime` is optional, so a record carries it for the Entries whose
// files had one and writes no key at all for the rest — and a replay gets both
// answers back the way they went in.
#[test]
fn a_birth_time_travels_only_with_the_entry_that_has_one() {
    let payload = encode(&record()).expect("encoding succeeds");
    let table = entry_table(&payload, PACK);
    assert!(
        !map_keys(&table[0]).contains(&"btime"),
        "an Entry whose file had no birth time carries no key for one",
    );

    let decoded = decode(&payload, generation(GENERATION)).expect("the payload reads back");
    let entries = decoded.additions()[PACK].entries();
    assert_eq!(entries[0].btime, None, "no birth time was ever captured");
    assert_eq!(entries[1].btime, Some(BORN));
}

/// Where the one addition with a two-Entry table lands, the record holding
/// `additions` in Container ID order (FM-15).
const PACK: usize = 1;

/// The entry table of one addition, as the encoder wrote it.
fn entry_table(payload: &ControlPayload, addition: usize) -> Vec<Value> {
    let mut fields = body_map(payload);
    let Value::Map(addition) = &mut array(&mut fields, "additions")[addition] else {
        panic!("an addition is a map");
    };
    let Value::Array(entries) = field(addition, "entries") else {
        panic!("an entry table is an array");
    };
    entries.clone()
}
