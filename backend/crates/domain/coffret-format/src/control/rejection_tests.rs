//! Control objects that are refused — on their shape, their name, or their tag.

use coffret_model::{ControlObjectKind, Generation, MasterKey, ReplicaPosition};

use super::decode::decode_control_object;
use super::encode::encode_control_object;
use super::encode_request::ControlEncodeRequest;
use super::header::ControlHeader;
use super::object_name::ControlObjectName;
use super::payload::ControlPayload;
use super::testing::{encode_with, epoch, key, master_key, name, payload, ALL_KINDS, SET_DIGEST};
use crate::error::Error;
use crate::purpose::Purpose;
use crate::purpose_key::PurposeKey;

/// A purpose key of the wrong kind, for tests that need one.
fn other_key() -> PurposeKey {
    PurposeKey::derive(&master_key(), Purpose::ContainerWrap)
}

// FM-11: the associated data is the full 44-byte header, so editing any byte of
// it — kind, generation, replica position, or nonce — fails decryption.
#[test]
fn tampering_with_any_header_field_fails_decryption() {
    // Every field of the header, by the offsets FM-11 lays down. The magic, the
    // version byte, and the reserved byte are rejected on shape instead, and the
    // kind byte moves the object to another kind's key; each has its own test.
    let fields: &[(&str, std::ops::Range<usize>)] = &[
        ("generation", 8..16),
        ("replica index", 16..18),
        ("replica count", 18..20),
        ("nonce", 20..44),
    ];
    let encoded = encode_with(ControlObjectKind::Keyring);
    let key = key(ControlObjectKind::Keyring);

    for (field, range) in fields {
        let mut object = encoded.bytes().to_vec();
        // Flipping the low bit of the last byte keeps every field a legal value:
        // generation 6 becomes 7, replica 1 of 3 becomes 0 of 3 or 1 of 2.
        object[range.end - 1] ^= 0x01;

        // The name is regenerated from the tampered header, so this is the
        // strongest form of the attack: a storage provider that renamed the
        // object to match its edit still cannot make it open.
        let renamed = ControlHeader::parse(&object).expect("the header is still well formed");
        let renamed =
            ControlObjectName::keyring_replica(renamed.generation, SET_DIGEST, renamed.replica)
                .expect("the digest is lowercase hex");

        assert_eq!(
            decode_control_object(&object, &renamed.to_string(), &key),
            Err(Error::AuthenticationFailed),
            "{field} was not authenticated"
        );
    }
}

// FM-11: the kind byte is part of the associated data too, so an object refiled
// as another kind fails decryption — even when it is renamed to match its new
// kind and presented to that kind's purpose key, which is the only way the
// framing would otherwise get as far as decrypting it.
#[test]
fn tampering_with_the_kind_byte_fails_decryption() {
    let mut object = encode_with(ControlObjectKind::Journal).into_bytes();
    // 0x01 (Journal) refiled as 0x03 (Index Snapshot), at the kind offset FM-11
    // lays down.
    object[6] = 0x03;

    assert_eq!(
        decode_control_object(
            &object,
            "idx-6.cfrt",
            &key(ControlObjectKind::IndexSnapshot)
        ),
        Err(Error::AuthenticationFailed)
    );
}

// FM-11: an object whose magic is not "CFCTL" is rejected without attempting
// decryption — the key here is never used.
#[test]
fn unknown_magic_is_rejected_before_decryption() {
    let mut object = encode_with(ControlObjectKind::Journal).into_bytes();
    object[..5].copy_from_slice(b"CFRT1");
    assert_eq!(
        decode_control_object(&object, "jrn-6.cfrt", &other_key()),
        Err(Error::UnknownControlMagic { actual: *b"CFRT1" })
    );
}

// FM-11: an object whose format version is unknown is rejected without
// attempting decryption.
#[test]
fn unknown_format_version_is_rejected_before_decryption() {
    let mut object = encode_with(ControlObjectKind::Journal).into_bytes();
    object[5] = 0x02;
    assert_eq!(
        decode_control_object(&object, "jrn-6.cfrt", &other_key()),
        Err(Error::UnsupportedControlVersion { actual: 0x02 })
    );
}

// FM-12: an object whose name-encoded kind disagrees with its header is
// rejected.
#[test]
fn a_name_of_the_wrong_kind_is_rejected() {
    let encoded = encode_with(ControlObjectKind::Journal);
    assert_eq!(
        decode_control_object(
            encoded.bytes(),
            "idx-6.cfrt",
            &key(ControlObjectKind::Journal)
        ),
        Err(Error::ObjectNameMismatch { field: "kind" })
    );
}

// FM-12: an object whose name-encoded generation disagrees with its header is
// rejected.
#[test]
fn a_name_of_the_wrong_generation_is_rejected() {
    let encoded = encode_with(ControlObjectKind::Journal);
    assert_eq!(
        decode_control_object(
            encoded.bytes(),
            "jrn-7.cfrt",
            &key(ControlObjectKind::Journal)
        ),
        Err(Error::ObjectNameMismatch {
            field: "generation"
        })
    );
}

// FM-12: an object whose name-encoded replica position disagrees with its header
// is rejected, so a replica cannot be filed into another slot of its set.
#[test]
fn a_name_in_the_wrong_replica_slot_is_rejected() {
    let encoded = encode_with(ControlObjectKind::Keyring);
    assert_eq!(
        decode_control_object(
            encoded.bytes(),
            &format!("key-6-{SET_DIGEST}-r2-of-3.cfrt"),
            &key(ControlObjectKind::Keyring)
        ),
        Err(Error::ObjectNameMismatch {
            field: "replica position"
        })
    );
}

#[test]
fn a_name_outside_the_forms_is_rejected() {
    let encoded = encode_with(ControlObjectKind::Journal);
    assert_eq!(
        decode_control_object(
            encoded.bytes(),
            "journal-6.cfrt",
            &key(ControlObjectKind::Journal)
        ),
        Err(Error::MalformedObjectName {
            name: "journal-6.cfrt".to_owned()
        })
    );
}

// KD-4: a payload is encrypted with the purpose key of its kind, so no other
// purpose key opens it — and none may write it either.
#[test]
fn only_the_kinds_own_purpose_key_opens_it() {
    for kind in ALL_KINDS {
        let encoded = encode_with(kind);
        let expected = Purpose::of_control_object(kind);

        for other in crate::purpose::ALL {
            if other == expected {
                continue;
            }
            let wrong = PurposeKey::derive(&master_key(), other);
            assert_eq!(
                decode_control_object(encoded.bytes(), encoded.object_name(), &wrong),
                Err(Error::WrongPurposeKey {
                    expected,
                    actual: other
                }),
                "{other} should not open a {kind:?} object"
            );
        }

        let name = name(kind);
        let payload = payload();
        let wrong = PurposeKey::derive(&master_key(), Purpose::ContainerWrap);
        assert!(matches!(
            encode_control_object(&ControlEncodeRequest::new(&name, &wrong, &payload)),
            Err(Error::WrongPurposeKey { .. })
        ));
    }
}

// KD-3: the purpose key is derived from the Master Key, so an object written
// under one Master Key does not open under another.
#[test]
fn another_master_keys_purpose_key_fails_authentication() {
    let encoded = encode_with(ControlObjectKind::Journal);
    let other = PurposeKey::derive(
        &MasterKey::from_bytes([0x01; MasterKey::BYTE_LEN]),
        Purpose::ControlJournal,
    );
    assert_eq!(
        decode_control_object(encoded.bytes(), encoded.object_name(), &other),
        Err(Error::AuthenticationFailed)
    );
}

// FM-13: a payload that does not carry `master_key_epoch` is rejected, even
// though it authenticates — the object cannot say which Master Key wrote it.
#[test]
fn a_payload_without_the_epoch_is_rejected() {
    // The epoch field is added by the framing, so writing a payload without one
    // means writing the map by hand: an empty map, sealed as the payload is.
    let name = ControlObjectName::journal(Generation::new(6));
    let key = key(ControlObjectKind::Journal);
    let mut empty_map = Vec::new();
    ciborium::into_writer(&ciborium::Value::Map(Vec::new()), &mut empty_map)
        .expect("an empty map serializes");

    let object = super::testing::seal_payload(&name, &key, &mut empty_map);
    assert_eq!(
        decode_control_object(&object, &name.to_string(), &key),
        Err(Error::MissingMasterKeyEpoch)
    );
}

// FM-13: the epoch a payload carries is the one it comes back with, whatever the
// header's generation says — the two count different things.
#[test]
fn the_epoch_is_independent_of_the_generation() {
    let name = ControlObjectName::journal(Generation::new(6));
    let key = key(ControlObjectKind::Journal);
    let payload = ControlPayload::empty(epoch(42));
    let encoded = encode_control_object(&ControlEncodeRequest::new(&name, &key, &payload))
        .expect("encoding succeeds");

    let decoded = decode_control_object(encoded.bytes(), encoded.object_name(), &key)
        .expect("the object is intact");
    assert_eq!(decoded.generation, Generation::new(6));
    assert_eq!(decoded.payload.master_key_epoch, epoch(42));
}

#[test]
fn an_object_with_no_payload_is_rejected() {
    let encoded = encode_with(ControlObjectKind::Journal);
    let header_only = encoded.bytes()[..ControlHeader::LEN].to_vec();
    assert_eq!(
        decode_control_object(
            &header_only,
            encoded.object_name(),
            &key(ControlObjectKind::Journal)
        ),
        Err(Error::MissingControlPayload)
    );
}

#[test]
fn a_truncated_payload_fails_authentication() {
    let encoded = encode_with(ControlObjectKind::Journal);
    let truncated = encoded.bytes()[..encoded.bytes().len() - 1].to_vec();
    assert_eq!(
        decode_control_object(
            &truncated,
            encoded.object_name(),
            &key(ControlObjectKind::Journal)
        ),
        Err(Error::AuthenticationFailed)
    );
}

// FM-11: a replica position the header itself contradicts is refused before any
// key is used, the same way an unknown kind byte is.
#[test]
fn a_header_with_an_impossible_replica_position_is_rejected() {
    let mut object = encode_with(ControlObjectKind::Keyring).into_bytes();
    object[16..18].copy_from_slice(&5u16.to_be_bytes());
    assert_eq!(
        decode_control_object(
            &object,
            &format!("key-6-{SET_DIGEST}-r1-of-3.cfrt"),
            &key(ControlObjectKind::Keyring)
        ),
        Err(Error::Model(coffret_model::Error::InvalidReplicaPosition {
            index: 5,
            count: 3
        }))
    );
    // The position the name claims is a legal one, so it is the header that is
    // refused, not the name.
    assert!(ReplicaPosition::new(1, 3).is_ok());
}
