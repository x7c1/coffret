//! What survives a trip through an Index Snapshot payload and back (FM-16).

use ciborium::Value;
use coffret_model::ControlObjectKind;

use super::testing::{activating, canonical, content, ordered_containers, ordinary, BORN, BORN_AT};
use super::{decode, encode, IndexSnapshotPayload};
use crate::control::testing::{array, body_keys, body_map, field, map_keys, with_body_map};

// FM-16, CK-1, CK-2, CK-3: an ordinary Snapshot's checkpoint, Containers, and
// Entries come back as they went in — with the Containers in ID order and the
// Entries in Entry Path order, which is what the encoder put them in.
#[test]
fn an_ordinary_snapshot_round_trips() {
    let payload = encode(&ordinary()).expect("encoding succeeds");
    let decoded = decode(&payload, ControlObjectKind::IndexSnapshot).expect("it reads back");
    assert_eq!(
        decoded,
        IndexSnapshotPayload::ordinary(canonical(content()))
    );
}

// MR-2: an activation Snapshot carries the same content and, beyond it, which
// head it fenced and the slot it won.
#[test]
fn an_activation_snapshot_round_trips() {
    let payload = encode(&activating()).expect("encoding succeeds");
    let decoded = decode(&payload, ControlObjectKind::ActivationSnapshot).expect("it reads back");
    let expected = activating();
    assert_eq!(decoded.activation, expected.activation);
    assert_eq!(decoded.content, canonical(expected.content));
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
    let decoded = decode(&encoded, ControlObjectKind::ActivationSnapshot).expect("it reads back");
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
        payload.content.adopted_from.is_some(),
        "this case needs content that has something to leave out"
    );
    let encoded = encode(&payload).expect("encoding succeeds");
    assert!(
        !body_keys(&encoded)
            .iter()
            .any(|key| key.contains("adopted")),
        "an Index Snapshot payload carries a field naming what it was adopted from"
    );

    let decoded = decode(&encoded, ControlObjectKind::IndexSnapshot).expect("it reads back");
    assert_eq!(decoded.content.adopted_from, None);
}

// FM-16: one Library state has one encoding, whatever order the Index reported
// its Containers and Entries in.
#[test]
fn the_same_content_in_a_different_order_encodes_identically() {
    let mut reordered = content();
    reordered.containers.reverse();
    reordered.entries.reverse();
    // Provenance is not content, so a Snapshot of the same Library adopted from
    // somewhere else is the same bytes (CK-7).
    reordered.adopted_from = None;

    let one = encode(&ordinary()).expect("encoding succeeds");
    let other = encode(&IndexSnapshotPayload::ordinary(reordered)).expect("encoding succeeds");
    assert_eq!(one.body, other.body);
}

// FM-16: an Entry names its Container by index into `containers`, so the
// 16-byte ID appears once per Container rather than once per Entry.
#[test]
fn an_entry_names_its_container_by_index() {
    let payload = encode(&ordinary()).expect("encoding succeeds");
    let mut fields = body_map(&payload);
    let containers = ordered_containers();
    let expected: Vec<u64> = canonical(content())
        .entries
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

// FM-9: the maps are forward-open, so a field a newer writer added is stepped
// over — at the Snapshot's own level, inside a Container, and inside an Entry.
#[test]
fn unknown_fields_are_ignored() {
    let payload = encode(&ordinary()).expect("encoding succeeds");
    let mut fields = body_map(&payload);
    fields.push((
        Value::Text("future_field".to_owned()),
        Value::Text("whatever".to_owned()),
    ));
    for array_name in ["containers", "entries"] {
        for item in array(&mut fields, array_name) {
            let Value::Map(map) = item else {
                panic!("{array_name} holds maps");
            };
            map.push((
                Value::Text("future_element_field".to_owned()),
                Value::from(1u64),
            ));
        }
    }
    *field(&mut fields, "schema") = Value::from(2u64);

    let extended = with_body_map(payload.master_key_epoch, fields);
    let decoded =
        decode(&extended, ControlObjectKind::IndexSnapshot).expect("unknown fields are ignored");
    assert_eq!(decoded.content, canonical(content()));
}

// FM-16: a Snapshot's entry map is the catalog's spelling, plus the `container`
// index that is the Snapshot's own — so `path` and `mtime` without the
// `original_` prefix FM-9 gives them, and `btime` for the Entries that have one.
#[test]
fn an_entry_carries_the_catalog_spelling_and_an_optional_birth_time() {
    let payload = encode(&ordinary()).expect("encoding succeeds");
    let mut fields = body_map(&payload);
    let entries = array(&mut fields, "entries").clone();

    assert_eq!(
        map_keys(&entries[BORN_AT]),
        [
            "path",
            "offset",
            "size",
            "mtime",
            "btime",
            "hash",
            "container"
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
    }

    let decoded = decode(&payload, ControlObjectKind::IndexSnapshot).expect("it reads back");
    assert_eq!(decoded.content.entries[BORN_AT].entry.btime, Some(BORN));
    assert_eq!(decoded.content.entries[BORN_AT + 1].entry.btime, None);
}
