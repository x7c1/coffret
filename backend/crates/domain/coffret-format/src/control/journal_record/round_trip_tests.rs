//! What survives a trip through a Journal record payload and back
//! (spec: FM-15).

use crate::field_numbers;
use ciborium::Value;
use coffret_model::{ContainerKind, Generation};

use super::testing::{
    addition, addition_of, first_record, first_record_of, record, record_of, renamed_record, table,
    BORN, CAPTURED_DIR, GENERATION,
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

// FM-15: a record's renames come back as they went in, each a map of the
// Container, the entry number and the new path — and nothing else, since the
// old name is the catalog's.
#[test]
fn a_record_with_renames_round_trips() {
    let record = renamed_record();
    let payload = encode(&record).expect("encoding succeeds");
    let decoded = decode(&payload, generation(GENERATION)).expect("the payload reads back");
    assert_eq!(decoded, record);
    assert_eq!(decoded.renames().len(), 3);

    let mut fields = body_map(&payload, &field_numbers::JOURNAL_RECORD);
    let renames = array(&mut fields, "renames");
    assert_eq!(map_keys(&renames[0]), ["container", "entry", "path"]);
}

// FM-15: a record that renames nothing leaves `renames` out rather than writing
// it empty, so a batch without renames costs nothing for them.
#[test]
fn a_record_without_renames_writes_no_renames_field() {
    let payload = encode(&record()).expect("encoding succeeds");
    assert!(
        !body_keys(&payload, &field_numbers::JOURNAL_RECORD).contains(&"renames".to_owned()),
        "renames was written for a record that renames nothing"
    );
}

// FM-15: Libraries created since the field-number format hold records of
// schema 2, which carry no `renames`. A reader still accepts them, as records
// that rename nothing.
#[test]
fn a_schema_2_record_reads_as_one_that_renames_nothing() {
    let payload = encode(&record()).expect("encoding succeeds");
    let mut fields = body_map(&payload, &field_numbers::JOURNAL_RECORD);
    *field(&mut fields, "schema") = Value::from(2u64);
    let payload = with_body_map(
        payload.master_key_epoch,
        fields,
        &field_numbers::JOURNAL_RECORD,
    );

    let decoded = decode(&payload, generation(GENERATION)).expect("a schema 2 record reads");
    assert_eq!(decoded, record());
    assert!(decoded.renames().is_empty());
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
            !body_keys(&payload, &field_numbers::JOURNAL_RECORD).contains(&absent.to_owned()),
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
    let written = body_keys(&payload, &field_numbers::JOURNAL_RECORD);
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

// FM-9: the maps are forward-open, so a field a newer writer added — under an
// field number this build does not know — is stepped
// over rather than refused — at the record's own level and inside an addition.
#[test]
fn unknown_fields_are_ignored() {
    let payload = encode(&record()).expect("encoding succeeds");
    let mut fields = body_map(&payload, &field_numbers::JOURNAL_RECORD);
    fields.push((Value::from(23u64), Value::Text("whatever".to_owned())));
    for addition in array(&mut fields, "additions") {
        let Value::Map(map) = addition else {
            panic!("an addition is a map");
        };
        map.push((Value::from(24u64), Value::from(1u64)));
    }
    // A newer writer would also have bumped `schema`.
    *field(&mut fields, "schema") = Value::from(3u64);

    let extended = with_body_map(
        payload.master_key_epoch,
        fields,
        &field_numbers::JOURNAL_RECORD,
    );
    let decoded = decode(&extended, generation(GENERATION)).expect("unknown fields are ignored");
    assert_eq!(decoded, record());
}

// CP-14: a removal is the Container ID and nothing else, so a removed Container
// costs sixteen bytes in a record however much it held.
#[test]
fn a_removal_is_the_container_id_alone() {
    let payload = encode(&record()).expect("encoding succeeds");
    let mut fields = body_map(&payload, &field_numbers::JOURNAL_RECORD);
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
// `mtime`, and an optional `btime`, without the `captured_` prefix FM-9 gives
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

// FM-15: an Entry's captured path travels beside its current one, each under
// its own field number, and each comes back as the one it went in as.
#[test]
fn a_captured_path_travels_beside_the_current_path() {
    let payload = encode(&record()).expect("encoding succeeds");
    let table = entry_table(&payload, PACK);
    let Value::Map(cover) = &table[0] else {
        panic!("an entry is a map");
    };
    let text = |key: &str| {
        cover
            .iter()
            .find(|(found, _)| found.as_text() == Some(key))
            .and_then(|(_, value)| value.as_text())
            .map(str::to_owned)
    };
    assert_eq!(text("path").as_deref(), Some("albums/40/cover.jpg"));
    assert_eq!(
        text("captured_path"),
        Some(format!("{CAPTURED_DIR}/40/cover.jpg"))
    );

    let decoded = decode(&payload, generation(GENERATION)).expect("the payload reads back");
    let entries = decoded.additions()[PACK].entries();
    assert_eq!(entries[0].path.as_str(), "albums/40/cover.jpg");
    assert_eq!(
        entries[0].captured_path.as_str(),
        format!("{CAPTURED_DIR}/40/cover.jpg")
    );
    assert_eq!(
        entries[1].captured_path, entries[1].path,
        "an Entry never moved carries its current path as its captured one",
    );
    let Value::Map(unmoved) = &table[1] else {
        panic!("an entry is a map");
    };
    assert!(
        !unmoved
            .iter()
            .any(|(key, _)| key.as_text() == Some("captured_path")),
        "an Entry whose two paths are one is written without `captured_path`",
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
/// `additions` in Container ID order (spec: FM-15).
const PACK: usize = 1;

/// The entry table of one addition, as the encoder wrote it.
fn entry_table(payload: &ControlPayload, addition: usize) -> Vec<Value> {
    let mut fields = body_map(payload, &field_numbers::JOURNAL_RECORD);
    let Value::Map(addition) = &mut array(&mut fields, "additions")[addition] else {
        panic!("an addition is a map");
    };
    let Value::Array(entries) = field(addition, "entries") else {
        panic!("an entry table is an array");
    };
    entries.clone()
}

// FM-15: on the wire every field of the record travels under its field number,
// and no map key is text, at any level.
#[test]
fn every_key_on_the_wire_is_an_integer() {
    fn integer_keyed(value: &Value) -> bool {
        match value {
            Value::Map(fields) => fields
                .iter()
                .all(|(key, value)| key.is_integer() && integer_keyed(value)),
            Value::Array(items) => items.iter().all(integer_keyed),
            _ => true,
        }
    }
    let payload = encode(&record()).expect("encoding succeeds");
    let body: Value = ciborium::from_reader(payload.body.as_slice()).expect("the body is CBOR");
    assert!(integer_keyed(&body), "{body:?}");
}
