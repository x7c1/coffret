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

/// The payload the objects these helpers seal all carry.
///
/// Named apart from the `payload` module the tests reach for beside it, so that
/// `payload::encode(…)` and this helper do not read as the same `payload`.
pub(super) fn sample_payload() -> ControlPayload {
    ControlPayload::new(epoch(2), body())
}

/// A payload whose map does not land on a Padmé bucket boundary, so the padding
/// the framing adds is there to be examined.
///
/// Which body that takes is not written down here: a field grows until the map
/// needs padding, so the helper still hands back a padded payload when the
/// fields around it change length.
pub(super) fn unaligned_payload() -> ControlPayload {
    for filler in 0..64 {
        let candidate = ControlPayload::new(epoch(2), filler_body(filler));
        let plaintext = super::payload::encode(&candidate).expect("encoding succeeds");
        if map_len(&plaintext) < plaintext.len() {
            return candidate;
        }
    }
    panic!("no payload body of this shape needed padding");
}

/// A body carrying one text field of `filler` characters.
fn filler_body(filler: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    ciborium::into_writer(
        &Value::Map(vec![(
            Value::Text("filler".to_owned()),
            Value::Text("f".repeat(filler)),
        )]),
        &mut bytes,
    )
    .expect("a map of text serializes");
    bytes
}

/// Where the CBOR map inside a payload plaintext ends.
///
/// Read the way a decoder reads it, since CBOR is self-delimiting and nothing in
/// the plaintext records it: take one item and see how much of it that took.
pub(super) fn map_len(plaintext: &[u8]) -> usize {
    let mut remaining = plaintext;
    let _: Value =
        ciborium::from_reader(&mut remaining).expect("the payload starts with a CBOR map");
    plaintext.len() - remaining.len()
}

/// An object of `kind`, sealed under that kind's purpose key.
pub(super) fn encode_with(kind: ControlObjectKind) -> EncodedControlObject {
    encode_payload_with(kind, &sample_payload())
}

/// An object of `kind` carrying `payload`, sealed under that kind's purpose key.
pub(super) fn encode_payload_with(
    kind: ControlObjectKind,
    payload: &ControlPayload,
) -> EncodedControlObject {
    let name = name(kind);
    let key = key(kind);
    encode_control_object(&ControlEncodeRequest::new(&name, kind, &key, payload))
        .expect("encoding a control object succeeds")
}

/// A payload map as the framing encrypts it: padded to its Padmé bucket
/// (FM-11).
///
/// Spelled out here rather than taken from the encoder, so a test that hands
/// [`seal_payload`] a hand-built map is padding it the way the rule says and not
/// the way this crate happens to.
pub(super) fn padded(mut map: Vec<u8>) -> Vec<u8> {
    let bucket = crate::padme::padded_len(map.len() as u64);
    map.resize(
        usize::try_from(bucket).expect("a test payload fits in memory"),
        0,
    );
    map
}

/// Frames `plaintext` as the payload of a `kind` object called `name`, whatever
/// it holds.
///
/// The encoder builds and pads its payload plaintext itself, so a test that
/// needs a payload the encoder would not write — one missing a field it always
/// adds, or one that was never padded — has to seal the bytes here instead.
pub(super) fn seal_payload(
    name: &ControlObjectName,
    kind: ControlObjectKind,
    key: &PurposeKey,
    plaintext: &[u8],
) -> Vec<u8> {
    let nonce = nonce::random().expect("the OS CSPRNG is available");
    let header = ControlHeader::new(kind, name.generation(), name.replica(), nonce);
    let header_bytes = header.to_bytes();
    let key = key
        .require(Purpose::of_control_object(kind))
        .expect("the key is of the object's kind");

    let mut object = header_bytes.to_vec();
    let mut plaintext = plaintext.to_vec();
    Cipher::new(key)
        .seal(&nonce, &header_bytes, &mut plaintext, &mut object)
        .expect("sealing succeeds");
    object
}
