//! Index Snapshot payloads a reader refuses (FM-16).

use ciborium::Value;
use coffret_model::{ContainerKind, ControlObjectKind, MAX_FORMAT_INTEGER};

use super::testing::{activating, content, content_holding, ordinary, GENERATION};
use super::{decode, encode, IndexSnapshotPayload};
use crate::control::testing::{array, body_map, field, summary, with_body_map};
use crate::error::Error;
use crate::generations::generation;
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
    read(payload, ControlObjectKind::IndexSnapshot)
}

/// The payload as a reader fetching it under the sample's own name meets it.
fn read(payload: &ControlPayload, kind: ControlObjectKind) -> crate::Result<IndexSnapshotPayload> {
    decode(payload, kind, generation(GENERATION))
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
    let result = read(&payload, ControlObjectKind::ActivationSnapshot);
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
    let result = read(&payload, ControlObjectKind::ActivationSnapshot);
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
        let result = read(&payload, kind);
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
    let content = content_holding(Vec::new(), Vec::new());
    let payload = encode(&IndexSnapshotPayload::ordinary(content)).expect("encoding succeeds");
    let decoded = read_ordinary(&payload).expect("an empty Library reads back");
    assert!(decoded.content.containers().is_empty());
    assert!(decoded.content.entries().is_empty());
}

// A Container the Library holds but no current Entry lives in is not an error:
// what makes a payload unreadable is an Entry without a Container, not the
// other way round.
#[test]
fn a_container_no_entry_names_is_kept() {
    let held = content();
    let mut containers = held.containers().to_vec();
    containers.push(summary(0xf0, ContainerKind::Pack));
    let content = content_holding(containers, held.entries().to_vec());

    let payload = encode(&IndexSnapshotPayload::ordinary(content)).expect("encoding succeeds");
    let decoded = read_ordinary(&payload).expect("it reads back");

    let empty = crate::control::testing::container_id(0xf0);
    assert!(
        decoded
            .content
            .containers()
            .iter()
            .any(|container| container.id == empty),
        "the Container no Entry names was dropped"
    );
    assert!(
        !decoded
            .content
            .entries()
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

// FM-9, FM-16, FM-19: a Snapshot lists every current Entry with the same values
// a Container's own entry table records, so a row whose `offset` and `size` end
// past the last plaintext stream position the format admits places nothing —
// and the Snapshot does not decode, with the refusal a meta section and a
// Journal record give the same row.
#[test]
fn an_entry_extent_past_the_end_of_the_address_space_is_rejected() {
    let payload = tampered(|fields| {
        *entry_field(fields, "offset") = Value::from(MAX_FORMAT_INTEGER);
    });
    let result = read_ordinary(&payload);
    assert!(
        matches!(result, Err(Error::StreamTooLong)),
        "expected an extent running past the address space to be refused, got {result:?}"
    );
}

// FM-19: an entry map's own numbers are integers the format bounds like any
// other, so one at the bound is a malformed Snapshot rather than a table that
// merely runs off the end — the object is refused either way, and the reason
// given is the one that applies.
//
// The detail names the field and the number, as every other malformed field of
// these schemas does: both are the format's own arithmetic and neither says
// anything about the Library's content.
#[test]
fn an_entry_integer_past_the_formats_integer_range_is_malformed() {
    let past_the_bound = MAX_FORMAT_INTEGER + 1;
    let payload = tampered(|fields| {
        *entry_field(fields, "offset") = Value::from(past_the_bound);
    });
    let result = read_ordinary(&payload);
    let Err(Error::MalformedIndexSnapshot { detail }) = result else {
        panic!("expected an offset of 2^63 to be malformed, got {result:?}");
    };
    assert!(detail.contains("offset"), "{detail}");
    assert!(detail.contains("below 2^63"), "{detail}");
    assert!(detail.contains(&past_the_bound.to_string()), "{detail}");
}

/// The value one key of the Snapshot's first entry holds.
fn entry_field<'a>(fields: &'a mut [(Value, Value)], key: &str) -> &'a mut Value {
    let Value::Map(entry) = &mut array(fields, "entries")[0] else {
        panic!("an entry is a CBOR map");
    };
    field(entry, key)
}

// CK-10, FM-13: a Snapshot checkpoints the head it is named for. One found at
// `idx-N` whose payload says it stands at another head would leave a device's
// checkpoint and its recorded starting point disagreeing, and this is the one
// place that is checked — for the ordinary kind and the activation kind alike,
// because a Library has one head chain across an epoch boundary.
#[test]
fn a_snapshot_that_checkpoints_another_head_is_rejected() {
    let payload = tampered(|fields| {
        *field(fields, "head_generation") = Value::from(GENERATION - 1);
        // The pair still holds CK-1 on its own, so what is refused here is the
        // disagreement with the name and nothing else.
        *field(fields, "journal_generation") = Value::from(GENERATION - 1);
    });
    let result = read_ordinary(&payload);
    assert!(
        matches!(
            result,
            Err(Error::SnapshotCheckpointsAnotherHead {
                generation: named,
                head_generation,
            }) if named == generation(GENERATION)
                && head_generation == generation(GENERATION - 1)
        ),
        "expected a Snapshot of another head to be refused, got {result:?}"
    );

    let activating = tampered_payload(&activating(), |fields| {
        *field(fields, "head_generation") = Value::from(GENERATION - 1);
        *field(fields, "journal_generation") = Value::from(GENERATION - 1);
    });
    let result = read(&activating, ControlObjectKind::ActivationSnapshot);
    assert!(
        matches!(result, Err(Error::SnapshotCheckpointsAnotherHead { .. })),
        "expected an activation Snapshot of another head to be refused, got {result:?}"
    );
}

// CK-1: the last applied Journal generation is never past the head it was
// applied to reach. A payload saying otherwise describes a state no commit
// produces, and a device that restored it would replay from a starting point
// its own checkpoint does not cover.
#[test]
fn a_checkpoint_whose_journal_is_ahead_of_its_head_is_rejected() {
    let payload = tampered(|fields| {
        *field(fields, "journal_generation") = Value::from(GENERATION + 1);
    });
    let result = read_ordinary(&payload);
    assert!(
        matches!(
            result,
            Err(Error::CheckpointJournalAheadOfHead {
                head_generation,
                journal_generation,
            }) if head_generation == generation(GENERATION)
                && journal_generation == generation(GENERATION + 1)
        ),
        "expected a Journal generation past the head to be refused, got {result:?}"
    );
}

// FM-16: the base head is the one whose commit slot the activation consumed
// (CP-3, MR-2), so it is a head the Library already reached — never the one
// this Snapshot takes, and never a later one.
#[test]
fn an_activation_naming_a_base_head_that_is_not_earlier_is_rejected() {
    for base in [GENERATION, GENERATION + 1] {
        let payload = tampered_payload(&activating(), |fields| {
            *field(fields, "base_head_generation") = Value::from(base);
        });
        let result = read(&payload, ControlObjectKind::ActivationSnapshot);
        assert!(
            matches!(
                result,
                Err(Error::ActivationBaseHeadNotEarlier {
                    head_generation,
                    base_head_generation,
                }) if head_generation == generation(GENERATION)
                    && base_head_generation == generation(base)
            ),
            "expected a base head of {base} to be refused, got {result:?}"
        );
    }
}
