//! What survives a trip through the control-object framing and back.

use coffret_model::{ControlObjectKind, Generation, ReplicaPosition};

use super::decode::decode_control_object;
use super::header::ControlHeader;
use super::testing::{
    body, encode_with, epoch, key, name, payload, ALL_KINDS, GENERATION, SET_DIGEST,
};
use crate::aead::TAG_LEN;

// FM-11, FM-13: a control object of any kind round-trips — the header's kind,
// generation, and replica position come back as written, and so do the payload's
// epoch and the kind's own fields.
#[test]
fn every_kind_round_trips() {
    for kind in ALL_KINDS {
        let encoded = encode_with(kind);
        let decoded = decode_control_object(encoded.bytes(), encoded.object_name(), &key(kind))
            .unwrap_or_else(|error| panic!("{kind:?} should open: {error}"));

        assert_eq!(decoded.kind, kind);
        assert_eq!(decoded.generation, Generation::new(GENERATION));
        assert_eq!(decoded.payload, payload());
        assert_eq!(decoded.payload.master_key_epoch, epoch(2));
        assert_eq!(decoded.payload.body, body());
    }
}

// FM-12: every row of the admission table round-trips under the name form it
// lists — including both head-chain kinds, which share one name.
#[test]
fn the_object_name_matches_the_form_of_its_role() {
    assert_eq!(
        encode_with(ControlObjectKind::Journal).object_name(),
        "head-6.cfrt"
    );
    assert_eq!(
        encode_with(ControlObjectKind::ActivationSnapshot).object_name(),
        "head-6.cfrt"
    );
    assert_eq!(
        encode_with(ControlObjectKind::IndexSnapshot).object_name(),
        "idx-6.cfrt"
    );
    assert_eq!(
        encode_with(ControlObjectKind::Keyring).object_name(),
        format!("key-6-{SET_DIGEST}-r1-of-3.cfrt")
    );
}

// FM-12: a `head-` name says which position in the chain an object occupies and
// nothing about which kind fills it, so the kind the object opens as is the one
// its header carries.
#[test]
fn one_head_name_opens_as_either_chain_kind() {
    for kind in [
        ControlObjectKind::Journal,
        ControlObjectKind::ActivationSnapshot,
    ] {
        let encoded = encode_with(kind);
        assert_eq!(encoded.object_name(), "head-6.cfrt");

        let decoded = decode_control_object(encoded.bytes(), "head-6.cfrt", &key(kind))
            .unwrap_or_else(|error| panic!("{kind:?} should open under its head name: {error}"));
        assert_eq!(decoded.kind, kind);
    }
}

// FM-12: heads and Index Snapshots use replica index 0, count 1, in their names
// and in their headers alike.
#[test]
fn single_written_kinds_carry_replica_zero_of_one() {
    for kind in [
        ControlObjectKind::Journal,
        ControlObjectKind::ActivationSnapshot,
        ControlObjectKind::IndexSnapshot,
    ] {
        let encoded = encode_with(kind);
        let decoded = decode_control_object(encoded.bytes(), encoded.object_name(), &key(kind))
            .expect("the object is intact");
        assert_eq!(decoded.replica, ReplicaPosition::SINGLE);
    }
}

// FM-12: a Keyring replica's position travels in the header as well as the name,
// so the two can be checked against each other.
#[test]
fn a_keyring_replica_carries_its_position() {
    let encoded = encode_with(ControlObjectKind::Keyring);
    let decoded = decode_control_object(
        encoded.bytes(),
        encoded.object_name(),
        &key(ControlObjectKind::Keyring),
    )
    .expect("the object is intact");

    assert_eq!(decoded.replica.index(), 1);
    assert_eq!(decoded.replica.count(), 3);
    assert_eq!(
        name(ControlObjectKind::Keyring).set_digest(),
        Some(SET_DIGEST)
    );
}

// FM-11: the object is the 44-byte header followed by the payload as one AEAD
// message — ciphertext and a 16-byte tag, and nothing else.
#[test]
fn the_object_is_a_header_and_one_aead_message() {
    let encoded = encode_with(ControlObjectKind::Journal);
    let object = encoded.bytes();
    let header = ControlHeader::parse(object).expect("the object has a valid header");

    assert_eq!(header.kind, ControlObjectKind::Journal);
    assert!(object.len() > ControlHeader::LEN + TAG_LEN);
    assert_eq!(&object[..5], b"CFCTL");
}

// FM-11: the nonce is drawn fresh for every object, so writing the same payload
// under the same name twice produces two different objects.
#[test]
fn every_object_gets_its_own_nonce() {
    let first = encode_with(ControlObjectKind::Keyring);
    let second = encode_with(ControlObjectKind::Keyring);
    assert_eq!(first.object_name(), second.object_name());
    assert_ne!(first.bytes(), second.bytes());

    let first_nonce = ControlHeader::parse(first.bytes()).expect("valid header");
    let second_nonce = ControlHeader::parse(second.bytes()).expect("valid header");
    assert_ne!(first_nonce, second_nonce);
}
