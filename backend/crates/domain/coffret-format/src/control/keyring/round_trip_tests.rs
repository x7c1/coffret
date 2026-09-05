//! What survives a trip through a Keyring payload and back (FM-17).

use ciborium::Value;
use coffret_model::{
    ContainerId, ContainerKeyStatus, ControlObjectName, KeyEnvelope, KeyringMapping,
    ReplicaPosition,
};

use super::set_digest::digest_input;
use super::testing::{mapping, mapping_epoch, mapping_of, pinned_mapping};
use super::{decode, encode, set_digest};
use crate::control::testing::{array, body_map, container_id, field, with_body_map};
use crate::generations::generation;

/// The digest of [`pinned_mapping`], which the TypeScript suite pins too.
///
/// Both implementations compute this from the same two entries, so a change to
/// what FM-17 hashes — the field order inside an element, the array order, the
/// CBOR spelling of a length — moves it here and in `keyring.test.ts` at once.
/// A digest that moved in only one of them is exactly the drift the interop
/// exchange exists to catch, caught before the exchange runs.
const PINNED_SET_DIGEST: &str = "6e6018ce7522ab4f82f4e43d51463efa48a0f57b1862d67b1a439c3d329c783a";

// FM-17, KL-7: both of the things a Keyring holds for a Container — an
// envelope and the explicit key-lost marker — come back as they went in, in
// the Container ID order the encoder put them in.
#[test]
fn a_mapping_of_envelopes_and_a_marker_round_trips() {
    let payload = encode(&mapping(), mapping_epoch()).expect("encoding succeeds");
    let decoded = decode(&payload).expect("it reads back");
    assert_eq!(decoded, mapping());
}

// A Library holding no Container yet still has a Keyring generation to
// commit: the mapping is empty, not missing.
#[test]
fn an_empty_mapping_round_trips() {
    let payload = encode(&KeyringMapping::default(), mapping_epoch()).expect("encoding succeeds");
    let decoded = decode(&payload).expect("an empty mapping reads back");
    assert!(decoded.entries().is_empty());
}

// FM-17: one mapping has one encoding, whatever order a caller held it in —
// which is what makes the digest below a property of the mapping rather than
// of the writer. The mapping holds its entries in that one order, so a caller
// handing them over reversed builds the same value.
#[test]
fn the_same_mapping_in_a_different_order_encodes_identically() {
    let mut reversed = mapping().entries().to_vec();
    reversed.reverse();
    let reordered = mapping_of(reversed);
    assert_eq!(reordered, mapping());

    let one = encode(&mapping(), mapping_epoch()).expect("encoding succeeds");
    let other = encode(&reordered, mapping_epoch()).expect("encoding succeeds");
    assert_eq!(one.body, other.body);
    assert_eq!(
        set_digest(&mapping()).expect("the digest is computed"),
        set_digest(&reordered).expect("the digest is computed")
    );
}

// KL-1, KL-14: the digest is a function of the mapping alone, so it is the same
// value every device computes for one generation — and it is pinned, because
// moving it silently would leave every name and commitment already written
// naming a set no reader can now match.
#[test]
fn the_digest_of_one_mapping_is_pinned() {
    assert_eq!(
        set_digest(&pinned_mapping()).expect("the digest is computed"),
        PINNED_SET_DIGEST
    );
}

// FM-17: what the digest above covers is deterministic CBOR, spelled out here
// byte by byte rather than taken from the encoder — the pin is only worth
// having if the bytes behind it are the ones the rule names, and an expectation
// read back out of the encoder would agree with any spelling it chose.
//
// Definite lengths, shortest-form arguments, and each element's keys in encoded
// order: `id` (two characters) before `envelope` or `key_lost` (eight).
#[test]
fn the_bytes_the_digest_covers_are_deterministic_cbor() {
    let mut expected = vec![0x82]; // an array of two elements
    expected.push(0xa2); // a map of two pairs
    expected.extend_from_slice(&[0x62, b'i', b'd']); // "id"
    expected.push(0x50); // a byte string of 16
    expected.extend_from_slice(&[0x11; ContainerId::BYTE_LEN]);
    expected.push(0x68); // "envelope"
    expected.extend_from_slice(b"envelope");
    expected.extend_from_slice(&[0x58, 0x48]); // a byte string of 72
    expected.extend_from_slice(&[0x22; KeyEnvelope::BYTE_LEN]);
    expected.push(0xa2); // the marker's element is a map of two pairs as well
    expected.extend_from_slice(&[0x62, b'i', b'd']);
    expected.push(0x50);
    expected.extend_from_slice(&[0x33; ContainerId::BYTE_LEN]);
    expected.push(0x68); // "key_lost"
    expected.extend_from_slice(b"key_lost");
    expected.push(0xf5); // true

    assert_eq!(
        digest_input(&pinned_mapping()).expect("the mapping serializes"),
        expected
    );
}

// FM-17: the digest covers the mapping, so it cannot also be inside it. The
// payload carries `mapping` and `schema` and nothing else.
#[test]
fn the_digest_is_not_a_field_of_the_payload() {
    let payload = encode(&mapping(), mapping_epoch()).expect("encoding succeeds");
    let keys: Vec<String> = body_map(&payload)
        .iter()
        .map(|(key, _)| key.as_text().expect("keys are text").to_owned())
        .collect();
    assert_eq!(keys, ["schema", "mapping"]);
}

// FM-12: the digest is the lowercase hex token a replica's name is built from,
// so the name builder takes what this returns without any further spelling.
#[test]
fn the_digest_is_the_token_a_replica_name_carries() {
    let digest = set_digest(&mapping()).expect("the digest is computed");
    let name = ControlObjectName::keyring_replica(
        generation(12),
        &digest,
        ReplicaPosition::new(1, 3).expect("replica 1 of 3 is a valid position"),
    )
    .expect("the digest a mapping produces is a valid one");
    assert_eq!(name.set_digest(), Some(digest.as_str()));
}

// FM-11, FM-13: the generation and the replica position are the header's, and
// the epoch is the framing's field, so none of the three is repeated in the
// map. The epoch still travels, on the payload the framing hands back.
#[test]
fn the_generation_the_replica_and_the_epoch_stay_in_the_framing() {
    let payload = encode(&mapping(), mapping_epoch()).expect("encoding succeeds");
    let fields = body_map(&payload);
    for absent in ["generation", "replica_index", "replica_count", "epoch"] {
        assert!(
            !fields.iter().any(|(key, _)| key.as_text() == Some(absent)),
            "the payload carries {absent}"
        );
    }
    assert_eq!(payload.master_key_epoch, mapping_epoch());
}

// KL-6: every replica of one generation carries the same payload, so the bytes
// a caller frames R times are encoded once.
#[test]
fn one_payload_serves_every_replica_of_a_generation() {
    let one = encode(&mapping(), mapping_epoch()).expect("encoding succeeds");
    let other = encode(&mapping(), mapping_epoch()).expect("encoding succeeds");
    assert_eq!(one.body, other.body);
}

// FM-9: the maps are forward-open, so a field a newer writer added is stepped
// over — at the payload's own level and inside an element.
#[test]
fn unknown_fields_are_ignored() {
    let payload = encode(&mapping(), mapping_epoch()).expect("encoding succeeds");
    let mut fields = body_map(&payload);
    fields.push((
        Value::Text("future_field".to_owned()),
        Value::Text("whatever".to_owned()),
    ));
    for element in array(&mut fields, "mapping") {
        let Value::Map(map) = element else {
            panic!("mapping holds maps");
        };
        map.push((
            Value::Text("future_element_field".to_owned()),
            Value::from(1u64),
        ));
    }
    *field(&mut fields, "schema") = Value::from(2u64);

    let extended = with_body_map(payload.master_key_epoch, fields);
    let decoded = decode(&extended).expect("unknown fields are ignored");
    assert_eq!(decoded, mapping());
}

// KL-7: an envelope and a marker are different answers about one Container, and
// a reader keeps them apart rather than collapsing a marker into "no envelope".
#[test]
fn a_marker_is_read_back_as_a_marker_and_not_as_an_absence() {
    let payload = encode(&mapping(), mapping_epoch()).expect("encoding succeeds");
    let decoded = decode(&payload).expect("it reads back");
    let lost = decoded
        .entries()
        .iter()
        .find(|entry| entry.container_id == container_id(0x99))
        .expect("the key-lost Container is mapped");
    assert_eq!(lost.key, ContainerKeyStatus::KeyLost);
}
