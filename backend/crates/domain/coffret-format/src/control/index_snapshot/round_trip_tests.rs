//! What survives a trip through an Index Snapshot payload and back
//! (spec: FM-16).

use crate::field_numbers;
use ciborium::Value;
use coffret_model::ControlObjectKind;

use super::testing::{
    activating, decoded_content, ordered_containers, ordinary, BORN, BORN_AT, CAPTURED_PATH,
    GENERATION,
};
use super::{decode, encode, IndexSnapshotPayload, SCHEMA};
use crate::control::testing::{array, body_keys, body_map, field, map_keys, with_body_map};
use crate::generations::generation;

// FM-16, CK-1, CK-2, CK-3: an ordinary Snapshot's checkpoint, Containers, and
// Entries come back as they went in — with the Containers in ID order and the
// Entries in Entry Path order, which is the order the content holds them in.
#[test]
fn an_ordinary_snapshot_round_trips() {
    let payload = encode(&ordinary()).expect("encoding succeeds");
    let decoded = read(&payload, ControlObjectKind::IndexSnapshot).expect("it reads back");
    assert_eq!(decoded, IndexSnapshotPayload::ordinary(decoded_content()));
}

// MR-2: an activation Snapshot carries the same content and, beyond it, which
// head it fenced and the slot it won.
#[test]
fn an_activation_snapshot_round_trips() {
    let payload = encode(&activating()).expect("encoding succeeds");
    let decoded = read(&payload, ControlObjectKind::ActivationSnapshot).expect("it reads back");
    assert_eq!(decoded.activation, activating().activation);
    assert_eq!(decoded.content, decoded_content());
}

// CP-2, CP-15: a name-keyed Storage persists no token, so an activation
// Snapshot from one carries a `base_head_generation` and no `activation_slot`
// — and that absence is not what tells the two Snapshot kinds apart.
#[test]
fn an_activation_snapshot_without_a_minted_slot_round_trips() {
    let mut payload = activating();
    payload
        .activation
        .as_mut()
        .expect("this one activates")
        .activation_slot = None;
    let encoded = encode(&payload).expect("encoding succeeds");
    let decoded = read(&encoded, ControlObjectKind::ActivationSnapshot).expect("it reads back");
    let activation = decoded.activation.expect("it still activates");
    assert_eq!(activation.activation_slot, None);
    assert_eq!(
        activation.base_head_generation,
        payload
            .activation
            .expect("this one activates")
            .base_head_generation
    );
}

// FM-16: the kind an object is framed as follows from the payload rather than
// from a flag beside it, so a caller cannot seal activation content as an
// ordinary Snapshot by mistake.
#[test]
fn the_payload_says_which_kind_it_has_to_be_framed_as() {
    assert_eq!(
        ordinary().control_object_kind(),
        ControlObjectKind::IndexSnapshot
    );
    assert_eq!(
        activating().control_object_kind(),
        ControlObjectKind::ActivationSnapshot
    );
}

// CK-7: a Snapshot carries no device state, so which checkpoint object this
// Index adopted is never encoded — there is no field for it at all — and a
// decoded Snapshot reports none.
#[test]
fn adopted_from_is_neither_written_nor_read_back() {
    let payload = ordinary();
    assert!(
        payload.content.adopted_from().is_some(),
        "this case needs content that has something to leave out"
    );
    let encoded = encode(&payload).expect("encoding succeeds");
    assert!(
        !body_keys(&encoded, &field_numbers::INDEX_SNAPSHOT)
            .iter()
            .any(|key| key.contains("adopted")),
        "an Index Snapshot payload carries a field naming what it was adopted from"
    );

    let decoded = read(&encoded, ControlObjectKind::IndexSnapshot).expect("it reads back");
    assert_eq!(decoded.content.adopted_from(), None);
}

// FM-16: one Library state has one encoding, whatever order the Index reported
// its Containers and Entries in — because the content holds them in the one
// order, whichever order it was handed. Provenance is not content, so a
// Snapshot of the same Library adopted from somewhere else is the same bytes
// (spec: CK-7).
#[test]
fn the_same_content_in_a_different_order_encodes_identically() {
    let one = encode(&ordinary()).expect("encoding succeeds");
    let other =
        encode(&IndexSnapshotPayload::ordinary(decoded_content())).expect("encoding succeeds");
    assert_eq!(one.body, other.body);
}

// FM-16: an Entry names its Container by index into `containers`, so the
// 16-byte ID appears once per Container rather than once per Entry.
#[test]
fn an_entry_names_its_container_by_index() {
    let payload = encode(&ordinary()).expect("encoding succeeds");
    let mut fields = body_map(&payload, &field_numbers::INDEX_SNAPSHOT);
    let containers = ordered_containers();
    let expected: Vec<u64> = decoded_content()
        .entries()
        .iter()
        .map(|location| {
            containers
                .iter()
                .position(|container| container.id == location.container_id)
                .expect("every Entry's Container is listed") as u64
        })
        .collect();

    let written: Vec<u64> = array(&mut fields, "entries")
        .iter()
        .map(|entry| {
            let Value::Map(map) = entry else {
                panic!("an entry is a map");
            };
            map.iter()
                .find(|(key, _)| key.as_text() == Some("container"))
                .and_then(|(_, value)| value.as_integer())
                .and_then(|integer| u64::try_from(integer).ok())
                .expect("every entry names its Container")
        })
        .collect();
    assert_eq!(written, expected);

    // And the ID itself is not repeated per Entry.
    for entry in array(&mut fields, "entries") {
        let Value::Map(map) = entry else {
            panic!("an entry is a map");
        };
        assert!(
            !map.iter().any(|(key, _)| key.as_text() == Some("id")),
            "an entry repeats its Container's ID"
        );
    }
}

// FM-9: the maps are forward-open, so a field a newer writer added — under an
// field number this build does not know — is stepped
// over — at the Snapshot's own level, inside a Container, and inside an Entry.
#[test]
fn unknown_fields_are_ignored() {
    let payload = encode(&ordinary()).expect("encoding succeeds");
    let mut fields = body_map(&payload, &field_numbers::INDEX_SNAPSHOT);
    fields.push((Value::from(23u64), Value::Text("whatever".to_owned())));
    for array_name in ["containers", "entries"] {
        for item in array(&mut fields, array_name) {
            let Value::Map(map) = item else {
                panic!("{array_name} holds maps");
            };
            map.push((Value::from(24u64), Value::from(1u64)));
        }
    }
    *field(&mut fields, "schema") = Value::from(SCHEMA + 1);

    let extended = with_body_map(
        payload.master_key_epoch,
        fields,
        &field_numbers::INDEX_SNAPSHOT,
    );
    let decoded =
        read(&extended, ControlObjectKind::IndexSnapshot).expect("unknown fields are ignored");
    assert_eq!(decoded.content, decoded_content());
}

// FM-16: a Snapshot's entry map is the catalog's spelling, plus the `container`
// index and the `entry` number that are the Snapshot's own — so `path` and
// `mtime` without the `captured_` prefix FM-9 gives them, and `btime` for the
// Entries that have one.
#[test]
fn an_entry_carries_the_catalog_spelling_and_an_optional_birth_time() {
    let payload = encode(&ordinary()).expect("encoding succeeds");
    let mut fields = body_map(&payload, &field_numbers::INDEX_SNAPSHOT);
    let entries = array(&mut fields, "entries").clone();

    assert_eq!(
        map_keys(&entries[BORN_AT]),
        [
            "path",
            "captured_path",
            "offset",
            "size",
            "mtime",
            "btime",
            "hash",
            "deposited_at",
            "container",
            "entry"
        ],
    );
    for (index, entry) in entries.iter().enumerate() {
        if index == BORN_AT {
            continue;
        }
        assert!(
            !map_keys(entry).contains(&"btime"),
            "entry {index} carries a birth time no file of its reported",
        );
        assert!(
            !map_keys(entry).contains(&"captured_path"),
            "entry {index} was never renamed, so it carries no captured path",
        );
    }

    let decoded = read(&payload, ControlObjectKind::IndexSnapshot).expect("it reads back");
    assert_eq!(decoded.content.entries()[BORN_AT].entry.btime, Some(BORN));
    assert_eq!(decoded.content.entries()[BORN_AT + 1].entry.btime, None);
}

// FM-16: each Entry carries the deposit time of its content, its own and no
// other Entry's, and it comes back as it went in.
#[test]
fn an_entry_carries_its_deposit_time() {
    let snapshot = ordinary();
    let payload = encode(&snapshot).expect("encoding succeeds");
    let decoded = read(&payload, ControlObjectKind::IndexSnapshot).expect("it reads back");
    let times = |content: &coffret_model::SnapshotContent| {
        content
            .entries()
            .iter()
            .map(|location| location.deposited_at)
            .collect::<Vec<_>>()
    };
    let written = times(&snapshot.content);
    assert_eq!(times(&decoded.content), written);
    let mut distinct = written.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        written.len(),
        "the fixture tells every Entry's time apart"
    );
}

// FM-9, FM-16: each Entry carries its entry number in its Container, which the
// Snapshot's path order would otherwise lose, and it comes back as it went in.
#[test]
fn an_entry_carries_its_entry_number() {
    let payload = encode(&ordinary()).expect("encoding succeeds");
    let mut fields = body_map(&payload, &field_numbers::INDEX_SNAPSHOT);
    let numbers: Vec<u64> = array(&mut fields, "entries")
        .iter()
        .map(|entry| {
            let Value::Map(entry) = entry else {
                panic!("an entry is a map");
            };
            entry
                .iter()
                .find(|(key, _)| key.as_text() == Some("entry"))
                .and_then(|(_, value)| value.as_integer())
                .and_then(|number| u64::try_from(number).ok())
                .expect("every entry carries its entry number")
        })
        .collect();
    let expected: Vec<u64> = ordinary()
        .content
        .entries()
        .iter()
        .map(|location| u64::from(location.entry_number.get()))
        .collect();
    assert_eq!(numbers, expected);
    assert!(
        numbers.contains(&1),
        "the sample has an Entry past the first of its Pack"
    );

    let decoded = read(&payload, ControlObjectKind::IndexSnapshot).expect("it reads back");
    assert_eq!(decoded.content.entries(), ordinary().content.entries());
}

// FM-16: an Entry's captured path travels beside its current one, each under
// its own field number, and each comes back as the one it went in as.
#[test]
fn a_captured_path_travels_beside_the_current_path() {
    let payload = encode(&ordinary()).expect("encoding succeeds");
    let mut fields = body_map(&payload, &field_numbers::INDEX_SNAPSHOT);
    let Value::Map(renamed) = &array(&mut fields, "entries")[BORN_AT] else {
        panic!("an entry is a map");
    };
    assert_eq!(
        renamed
            .iter()
            .find(|(key, _)| key.as_text() == Some("captured_path"))
            .and_then(|(_, value)| value.as_text()),
        Some(CAPTURED_PATH),
    );

    let decoded = read(&payload, ControlObjectKind::IndexSnapshot).expect("it reads back");
    let entries = decoded.content.entries();
    assert_eq!(entries[BORN_AT].entry.path.as_str(), "albums/spring/a.jpg");
    assert_eq!(entries[BORN_AT].entry.captured_path.as_str(), CAPTURED_PATH);
    for (index, location) in entries.iter().enumerate() {
        if index != BORN_AT {
            assert_eq!(location.entry.captured_path, location.entry.path);
        }
    }
}

/// The payload as a reader fetching it under the sample's own name meets it.
fn read(
    payload: &crate::control::ControlPayload,
    kind: ControlObjectKind,
) -> crate::Result<IndexSnapshotPayload> {
    decode(payload, kind, generation(GENERATION))
}

// FM-16: a checkpoint carries the Library's name the records it checkpoints
// left, so a device that catches up from the Snapshot alone reads it — and a
// Snapshot of schema 4, written before one could carry a name, reads as one
// naming nothing.
#[test]
fn a_snapshot_carries_the_library_name() {
    let name = coffret_model::LibraryName::parse("books").expect("a fixture's name is one");
    let named =
        IndexSnapshotPayload::ordinary(super::testing::content().with_library_name(Some(name)));
    let payload = encode(&named).expect("encoding succeeds");
    let decoded = read(&payload, ControlObjectKind::IndexSnapshot).expect("it reads back");
    assert_eq!(
        decoded.content.library_name().map(|name| name.as_str()),
        Some("books")
    );

    let unnamed = encode(&ordinary()).expect("encoding succeeds");
    let mut fields = body_map(&unnamed, &field_numbers::INDEX_SNAPSHOT);
    assert!(!fields
        .iter()
        .any(|(key, _)| key.as_text() == Some("library_name")));
    *field(&mut fields, "schema") = Value::from(4u64);
    let older = with_body_map(
        unnamed.master_key_epoch,
        fields,
        &field_numbers::INDEX_SNAPSHOT,
    );
    let decoded =
        read(&older, ControlObjectKind::IndexSnapshot).expect("a schema-4 Snapshot reads");
    assert_eq!(decoded.content.library_name(), None);
}
