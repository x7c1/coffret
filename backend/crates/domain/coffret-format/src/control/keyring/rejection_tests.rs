//! Keyring payloads a reader refuses (FM-17).

use ciborium::Value;

use super::testing::{envelope, mapping, mapping_epoch};
use super::{decode, encode};
use crate::control::testing::{array, body_map, field, with_body_map};
use crate::error::Error;
use crate::ControlPayload;

/// A Keyring payload with one thing changed by hand, as a reader meets it.
fn tampered(change: impl FnOnce(&mut Vec<(Value, Value)>)) -> ControlPayload {
    let payload = encode(&mapping(), mapping_epoch()).expect("encoding succeeds");
    let mut fields = body_map(&payload);
    change(&mut fields);
    with_body_map(payload.master_key_epoch, fields)
}

/// The fields of one element of `mapping`, for a case that has to change them.
fn element(fields: &mut [(Value, Value)], index: usize) -> &mut Vec<(Value, Value)> {
    match &mut array(fields, "mapping")[index] {
        Value::Map(map) => map,
        other => panic!("an element of mapping is a map, found {other:?}"),
    }
}

// FM-17, KL-7: an envelope says the Container opens and the marker says no
// envelope is reachable. An element carrying both says both, which is not a
// state a Container can be in.
#[test]
fn an_element_with_both_an_envelope_and_a_marker_is_rejected() {
    let payload = tampered(|fields| {
        element(fields, 0).push((Value::Text("key_lost".to_owned()), Value::Bool(true)));
    });
    let result = decode(&payload);
    assert!(
        matches!(
            result,
            Err(Error::KeyringEntryWithEnvelopeAndMarker { index: 0 })
        ),
        "expected an element carrying both to be refused, got {result:?}"
    );
}

// The other way round: an element that says nothing about its Container maps it
// to no determinate state, and a mapping of such elements could not be the
// complete one KL-7 obliges.
#[test]
fn an_element_with_neither_an_envelope_nor_a_marker_is_rejected() {
    let payload = tampered(|fields| {
        element(fields, 0).retain(|(key, _)| key.as_text() != Some("envelope"));
    });
    let result = decode(&payload);
    assert!(
        matches!(
            result,
            Err(Error::KeyringEntryWithoutEnvelopeOrMarker { index: 0 })
        ),
        "expected an element carrying neither to be refused, got {result:?}"
    );
}

// FM-17: the marker is spelled `true`, so `false` is a writer stating the field
// in a form the rule does not define — not a way of saying there is no marker.
#[test]
fn a_key_lost_marker_that_is_not_true_is_rejected() {
    let payload = tampered(|fields| {
        let element = element(fields, 2);
        element.retain(|(key, _)| key.as_text() != Some("key_lost"));
        element.push((Value::Text("key_lost".to_owned()), Value::Bool(false)));
    });
    let result = decode(&payload);
    assert!(
        matches!(result, Err(Error::KeyringEntryMarkerNotTrue { index: 2 })),
        "expected a false marker to be refused, got {result:?}"
    );
}

// FM-17: `mapping` is in Container ID order so that one mapping has one
// encoding and therefore one `set_digest`. A payload out of that order is
// refused rather than sorted: sorting it would accept a second encoding of one
// state, whose digest no name and no commitment matches.
#[test]
fn a_mapping_out_of_id_order_is_rejected() {
    let payload = tampered(|fields| array(fields, "mapping").reverse());
    let result = decode(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ControlPayloadOutOfOrder {
                array: "mapping",
                index: 1
            })
        ),
        "expected a reversed mapping to be refused, got {result:?}"
    );
}

// KL-7: one Container has one entry in the mapping, so an ID listed twice is
// not a sorted mapping with a repeat in it — it is a payload holding two
// answers about one Container.
#[test]
fn one_container_mapped_twice_is_rejected() {
    let payload = tampered(|fields| {
        let mapping = array(fields, "mapping");
        mapping[1] = mapping[0].clone();
    });
    let result = decode(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ControlPayloadOutOfOrder {
                array: "mapping",
                index: 1
            })
        ),
        "expected a Container mapped twice to be refused, got {result:?}"
    );
}

// FM-14: an envelope is 72 bytes, and a field of another length carried the
// shape the schema gives it but not a value the type accepts.
#[test]
fn an_envelope_that_is_not_the_length_fm_14_gives_it_is_rejected() {
    let payload = tampered(|fields| {
        let element = element(fields, 0);
        *field(element, "envelope") = Value::Bytes(vec![0x11; 71]);
    });
    let result = decode(&payload);
    assert!(
        matches!(
            result,
            Err(Error::Model(coffret_model::Error::InvalidByteLength {
                expected: 72,
                actual: 71
            }))
        ),
        "expected a 71-byte envelope to be refused, got {result:?}"
    );
}

#[test]
fn a_schema_below_one_is_rejected() {
    let payload = tampered(|fields| *field(fields, "schema") = Value::from(0u64));
    let result = decode(&payload);
    assert!(
        matches!(result, Err(Error::UnsupportedKeyringSchema { schema: 0 })),
        "expected schema 0 to be unreadable, got {result:?}"
    );
}

#[test]
fn a_missing_mapping_is_reported_by_name() {
    let payload = tampered(|fields| fields.retain(|(key, _)| key.as_text() != Some("mapping")));
    let result = decode(&payload);
    assert!(
        matches!(result, Err(Error::MalformedKeyringPayload { ref detail }) if detail.contains("mapping")),
        "expected the missing field to be named, got {result:?}"
    );
}

// A payload naming a Container without saying which one is unreadable in the
// same way: the field is named, and what stood in its place is not quoted.
#[test]
fn an_element_without_an_id_is_reported_by_name() {
    let payload = tampered(|fields| {
        element(fields, 0).retain(|(key, _)| key.as_text() != Some("id"));
    });
    let result = decode(&payload);
    assert!(
        matches!(result, Err(Error::MalformedKeyringPayload { ref detail }) if detail.contains("id")),
        "expected the missing field to be named, got {result:?}"
    );
}

// An element that is not a map at all is refused before any field is looked
// for, so a payload of the wrong shape does not read as an empty element.
#[test]
fn an_element_that_is_not_a_map_is_rejected() {
    let payload = tampered(|fields| {
        array(fields, "mapping")[0] = Value::Bytes(envelope(0x40).as_bytes().to_vec());
    });
    let result = decode(&payload);
    assert!(
        matches!(result, Err(Error::MalformedKeyringPayload { .. })),
        "expected an element that is not a map to be refused, got {result:?}"
    );
}
