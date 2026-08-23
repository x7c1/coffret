//! Helpers shared by the control-object tests.

use ciborium::Value;
use coffret_model::{
    ControlObjectKind, ControlObjectName, Generation, MasterKey, MasterKeyEpoch, ReplicaPosition,
};

use super::encode::encode_control_object;
use super::encode_request::ControlEncodeRequest;
use super::encoded_object::EncodedControlObject;
use super::header::ControlHeader;
use super::payload::ControlPayload;
use crate::aead::Cipher;
use crate::nonce;
use crate::purpose::Purpose;
use crate::purpose_key::PurposeKey;

/// Every kind of control object, for tests that must cover all of them.
pub(super) const ALL_KINDS: [ControlObjectKind; 4] = [
    ControlObjectKind::Journal,
    ControlObjectKind::Keyring,
    ControlObjectKind::IndexSnapshot,
    ControlObjectKind::ActivationSnapshot,
];

/// The digest a Keyring replica name in these tests carries.
pub(super) const SET_DIGEST: &str = "9f0c";

/// The generation every name these helpers build sits at.
pub(super) const GENERATION: u64 = 6;

pub(super) fn master_key() -> MasterKey {
    MasterKey::from_bytes([0x7c; MasterKey::BYTE_LEN])
}

/// The purpose key that opens control objects of one kind.
pub(super) fn key(kind: ControlObjectKind) -> PurposeKey {
    PurposeKey::derive(&master_key(), Purpose::of_control_object(kind))
}

pub(super) fn epoch(value: u64) -> MasterKeyEpoch {
    MasterKeyEpoch::new(value).expect("the epoch is valid")
}

/// The name a control object of `kind` is stored under in these tests.
///
/// One generation throughout, so a test that swaps two names is swapping the
/// name form and nothing else. Both head-chain kinds land on the same name,
/// which is the point of FM-12's admission table.
pub(super) fn name(kind: ControlObjectKind) -> ControlObjectName {
    let generation = Generation::new(GENERATION);
    match kind {
        ControlObjectKind::Journal | ControlObjectKind::ActivationSnapshot => {
            ControlObjectName::head(generation)
        }
        ControlObjectKind::IndexSnapshot => ControlObjectName::index_snapshot(generation),
        ControlObjectKind::Keyring => ControlObjectName::keyring_replica(
            generation,
            SET_DIGEST,
            ReplicaPosition::new(1, 3).expect("replica 1 of 3 is a valid position"),
        )
        .expect("the digest is lowercase hex"),
    }
}

/// A payload body standing in for the fields a kind will carry later.
pub(super) fn body() -> Vec<u8> {
    let mut bytes = Vec::new();
    ciborium::into_writer(
        &Value::Map(vec![(
            Value::Text("placeholder".to_owned()),
            Value::from(1u64),
        )]),
        &mut bytes,
    )
    .expect("a map of integers serializes");
    bytes
}

pub(super) fn payload() -> ControlPayload {
    ControlPayload::new(epoch(2), body())
}

/// An object of `kind`, sealed under that kind's purpose key.
pub(super) fn encode_with(kind: ControlObjectKind) -> EncodedControlObject {
    let name = name(kind);
    let key = key(kind);
    let payload = payload();
    encode_control_object(&ControlEncodeRequest::new(&name, kind, &key, &payload))
        .expect("encoding a control object succeeds")
}

/// Frames `plaintext` as the payload of a `kind` object called `name`, whatever
/// it holds.
///
/// The encoder builds its payload map itself, so a test that needs a payload the
/// encoder would not write — one missing a field it always adds, say — has to
/// seal the bytes here instead.
pub(super) fn seal_payload(
    name: &ControlObjectName,
    kind: ControlObjectKind,
    key: &PurposeKey,
    plaintext: &mut [u8],
) -> Vec<u8> {
    let nonce = nonce::random().expect("the OS CSPRNG is available");
    let header = ControlHeader::new(kind, name.generation(), name.replica(), nonce);
    let header_bytes = header.to_bytes();
    let key = key
        .require(Purpose::of_control_object(kind))
        .expect("the key is of the object's kind");

    let mut object = header_bytes.to_vec();
    Cipher::new(key)
        .seal(&nonce, &header_bytes, plaintext, &mut object)
        .expect("sealing succeeds");
    object
}
