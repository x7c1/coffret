//! Index Snapshot payloads a reader refuses (FM-16).

use ciborium::Value;
use coffret_model::{ContainerKind, ControlObjectKind};

use super::testing::{activating, content, located, ordinary};
use super::{decode, encode, IndexSnapshotPayload};
use crate::control::testing::{array, body_map, field, summary, with_body_map};
use crate::error::Error;
use crate::ControlPayload;

/// An ordinary Snapshot payload with one field changed by hand.
fn tampered(change: impl FnOnce(&mut Vec<(Value, Value)>)) -> ControlPayload {
    tampered_payload(&ordinary(), change)
}

fn tampered_payload(
    snapshot: &IndexSnapshotPayload,
    change: impl FnOnce(&mut Vec<(Value, Value)>),
) -> ControlPayload {
    let payload = encode(snapshot).expect("encoding succeeds");
    let mut fields = body_map(&payload);
    change(&mut fields);
    with_body_map(payload.master_key_epoch, fields)
}

fn read_ordinary(payload: &ControlPayload) -> crate::Result<IndexSnapshotPayload> {
    decode(payload, ControlObjectKind::IndexSnapshot)
}

// FM-16: `containers` is in Container ID order so that one Library state has
// one encoding, and a payload that is not in it is refused rather than sorted.
#[test]
fn containers_out_of_id_order_are_rejected() {
    let payload = tampered(|fields| array(fields, "containers").reverse());
    let result = read_ordinary(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ControlPayloadOutOfOrder {
                array: "containers",
                index: 1
            })
        ),
        "expected reversed Containers to be refused, got {result:?}"
    );
}

// EP-3: `entries` is in the canonical byte order of the Entry Paths, which is
// what lets a prefix range be answered by binary search — a payload out of that
// order would answer such a range wrongly rather than slowly.
#[test]
fn entries_out_of_path_order_are_rejected() {
    let payload = tampered(|fields| array(fields, "entries").reverse());
    let result = read_ordinary(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ControlPayloadOutOfOrder {
                array: "entries",
                index: 1
            })
        ),
        "expected reversed Entries to be refused, got {result:?}"
    );
}

// EP-5: one Entry Path holds at most one current Entry, so a Snapshot naming a
// path twice is not a sorted Snapshot with a repeat in it.
#[test]
fn one_entry_path_listed_twice_is_rejected() {
    let payload = tampered(|fields| {
        let entries = array(fields, "entries");
        entries[1] = entries[0].clone();
    });
    let result = read_ordinary(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ControlPayloadOutOfOrder {
                array: "entries",
                ..
            })
        ),
        "expected a repeated Entry Path to be refused, got {result:?}"
    );
}

// FM-16: an Entry names its Container by index, and an index past the end of
// `containers` names no Container at all — the Snapshot cannot be read back
// into an Index, so it is refused rather than partly applied.
#[test]
fn an_entry_naming_a_container_past_the_end_is_rejected() {
    let payload = tampered(|fields| {
        let Value::Map(entry) = &mut array(fields, "entries")[0] else {
            panic!("an entry is a map");
        };
        *field(entry, "container") = Value::from(9u64);
    });
    let result = read_ordinary(&payload);
    assert!(
        matches!(
            result,
            Err(Error::DanglingContainerIndex {
                entry: 0,
                container: 9,
                containers: 3
            })
        ),
        "expected a dangling Container index to be refused, got {result:?}"
    );
}

// The same, on the way out: content whose Entry is held by a Container the
// Snapshot does not list has no index to write, and the encoder says so rather
// than writing a Snapshot no reader could take.
#[test]
fn writing_an_entry_whose_container_is_not_listed_is_refused() {
    let mut content = content();
    content.entries.push(located(0x77, "zzz/orphan.jpg", 0, 10));
    let result = encode(&IndexSnapshotPayload::ordinary(content));
    assert!(
        matches!(
            result,
            Err(Error::SnapshotEntryWithoutContainer { container_id, .. })
                if container_id == crate::control::testing::container_id(0x77)
        ),
        "expected an Entry without a Container to be refused, got {result:?}"
    );
}

// FM-16, MR-2: the activation fields are the activation kind's alone. An
// ordinary Snapshot carrying one was either written by something that does not
// follow the rule or moved from a head position, and either way the kind in the
// authenticated header and the payload disagree.
#[test]
fn an_ordinary_snapshot_carrying_activation_fields_is_rejected() {
    let payload = tampered(|fields| {
        fields.push((
            Value::Text("base_head_generation".to_owned()),
            Value::from(6u64),
        ));
    });
    let result = read_ordinary(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ActivationFieldOnOrdinarySnapshot {
                field: "base_head_generation"
            })
        ),
        "expected base_head_generation on an ordinary Snapshot to be refused, got {result:?}"
    );

    let payload = tampered(|fields| {
        fields.push((
            Value::Text("activation_slot".to_owned()),
            Value::Text("minted-head-7".to_owned()),
        ));
    });
    let result = read_ordinary(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ActivationFieldOnOrdinarySnapshot {
                field: "activation_slot"
            })
        ),
        "expected activation_slot on an ordinary Snapshot to be refused, got {result:?}"
    );
}

// The other direction: an object whose header says it activated an epoch has to
// say which head it fenced, or nothing records the fence at all (MR-2).
#[test]
fn an_activation_snapshot_without_the_head_it_fenced_is_rejected() {
    let payload = tampered_payload(&activating(), |fields| {
        fields.retain(|(key, _)| key.as_text() != Some("base_head_generation"));
    });
    let result = decode(&payload, ControlObjectKind::ActivationSnapshot);
    assert!(
        matches!(
            result,
            Err(Error::ActivationSnapshotFieldMissing {
                field: "base_head_generation"
            })
        ),
        "expected an activation Snapshot without its base head to be refused, got {result:?}"
    );
}

// An ordinary Snapshot's payload presented as an activation one is refused for
// the same reason, which is what keeps a renamed object from being read as the
// kind its new position implies.
#[test]
fn an_ordinary_payload_read_as_an_activation_snapshot_is_rejected() {
    let payload = encode(&ordinary()).expect("encoding succeeds");
    let result = decode(&payload, ControlObjectKind::ActivationSnapshot);
    assert!(
        matches!(result, Err(Error::ActivationSnapshotFieldMissing { .. })),
        "expected an ordinary payload under the activation kind to be refused, got {result:?}"
    );
}

// FM-11: only two of the four control-object kinds are Index Snapshots, so a
// Journal record's or a Keyring's payload is never read as one.
#[test]
fn a_kind_that_is_no_index_snapshot_is_refused_outright() {
    let payload = encode(&ordinary()).expect("encoding succeeds");
    for kind in [ControlObjectKind::Journal, ControlObjectKind::Keyring] {
        let result = decode(&payload, kind);
        assert!(
            matches!(result, Err(Error::NotAnIndexSnapshotKind { kind: refused }) if refused == kind),
            "expected {kind:?} to be refused, got {result:?}"
        );
    }
}

#[test]
fn a_schema_below_one_is_rejected() {
    let payload = tampered(|fields| *field(fields, "schema") = Value::from(0u64));
    let result = read_ordinary(&payload);
    assert!(
        matches!(
            result,
            Err(Error::UnsupportedIndexSnapshotSchema { schema: 0 })
        ),
        "expected schema 0 to be unreadable, got {result:?}"
    );
}

#[test]
fn a_missing_checkpoint_field_is_reported_by_name() {
    let payload = tampered(|fields| {
        fields.retain(|(key, _)| key.as_text() != Some("journal_generation"));
    });
    let result = read_ordinary(&payload);
    assert!(
        matches!(result, Err(Error::MalformedIndexSnapshot { ref detail })
            if detail.contains("journal_generation")),
        "expected the missing field to be named, got {result:?}"
    );
}

// PK-15: a Container's kind is the explicit one it recorded, so a spelling this
// format version has no kind for is refused rather than guessed at.
#[test]
fn a_container_of_an_unknown_kind_is_rejected() {
    let payload = tampered(|fields| {
        let Value::Map(container) = &mut array(fields, "containers")[0] else {
            panic!("a Container is a map");
        };
        *field(container, "kind") = Value::Text("archive".to_owned());
    });
    let result = read_ordinary(&payload);
    assert!(
        matches!(result, Err(Error::MalformedIndexSnapshot { ref detail }) if detail.contains("archive")),
        "expected an unknown kind to be refused, got {result:?}"
    );
}

// A Snapshot of a Library that holds nothing is still a Snapshot: it has a
// checkpoint to preserve, and no Entry to name a Container it does not list.
#[test]
fn a_snapshot_of_an_empty_library_round_trips() {
    let mut content = content();
    content.containers.clear();
    content.entries.clear();
    let payload = encode(&IndexSnapshotPayload::ordinary(content)).expect("encoding succeeds");
    let decoded = read_ordinary(&payload).expect("an empty Library reads back");
    assert!(decoded.content.containers.is_empty());
    assert!(decoded.content.entries.is_empty());
}

// A Container the Library holds but no current Entry lives in is not an error:
// what makes a payload unreadable is an Entry without a Container, not the
// other way round.
#[test]
fn a_container_no_entry_names_is_kept() {
    let mut content = content();
    content.containers.push(summary(0xf0, ContainerKind::Pack));
    let payload = encode(&IndexSnapshotPayload::ordinary(content)).expect("encoding succeeds");
    let decoded = read_ordinary(&payload).expect("it reads back");

    let empty = crate::control::testing::container_id(0xf0);
    assert!(
        decoded
            .content
            .containers
            .iter()
            .any(|container| container.id == empty),
        "the Container no Entry names was dropped"
    );
    assert!(
        !decoded
            .content
            .entries
            .iter()
            .any(|location| location.container_id == empty),
        "an Entry was invented for the empty Container"
    );
}

// EP-2: a Snapshot's entries carry paths the Library already holds, so one
// outside the shape every Entry Path is in was written by something that did not
// hold to EP-2 — the Snapshot does not decode, and the catalog it would have
// been read into is rebuilt from Storage instead (spec: RV-5).
#[test]
fn an_entry_path_with_a_shape_ep_2_excludes_is_rejected() {
    let payload = tampered(|fields| {
        let Value::Map(entry) = &mut array(fields, "entries")[0] else {
            panic!("an entry is a CBOR map");
        };
        *field(entry, "path") = Value::Text("../x".to_owned());
    });
    let result = read_ordinary(&payload);
    assert!(
        matches!(result, Err(Error::MalformedEntryPath { field: "path" })),
        "expected a `..` component to be refused, got {result:?}"
    );
}
