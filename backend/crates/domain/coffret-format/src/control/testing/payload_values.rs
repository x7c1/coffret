//! The values the payload schemas are built out of (FM-15, FM-16, FM-17), and
//! the helpers a case reads one of their maps back with.
//!
//! The values are deliberately dull: a Container ID is one byte repeated and a
//! hash is another, because none of what the payload cases assert turns on what
//! is in them. What matters is that two of them are distinguishable at a glance
//! in a failure message, and that their IDs are *not* in the order a test hands
//! them over in, so an encoder that left the order alone would be caught.

use ciborium::Value;
use coffret_model::{
    ContainerId, ContainerKind, ContainerSummary, ContentHash, EntryMetadata, EntryPath,
    Generation, IndexCheckpoint, KeyringCommitment, MasterKeyEpoch, Mtime, ObjectRef,
};

use super::epoch;
use crate::control::ControlPayload;

/// A Container ID whose sixteen bytes are all `seed`.
pub(in crate::control) fn container_id(seed: u8) -> ContainerId {
    ContainerId::from_bytes([seed; ContainerId::BYTE_LEN])
}

/// A content hash whose thirty-two bytes are all `seed`.
pub(in crate::control) fn content_hash(seed: u8) -> ContentHash {
    ContentHash::from_bytes([seed; ContentHash::BYTE_LEN])
}

/// What a payload records about one Container.
///
/// An even seed caches the provider's handle for the object and an odd one does
/// not, so a set of two covers both spellings of the optional field (CP-11).
pub(in crate::control) fn summary(seed: u8, kind: ContainerKind) -> ContainerSummary {
    ContainerSummary {
        id: container_id(seed),
        kind,
        ciphertext_hash: content_hash(seed),
        ciphertext_len: 4096 + u64::from(seed),
        object_ref: seed
            .is_multiple_of(2)
            .then(|| ObjectRef::new(format!("stored-{seed}"))),
    }
}

/// One entry-table element, laid at `offset` and `size` bytes long (FM-9).
pub(in crate::control) fn entry(path: &str, offset: u64, size: u64) -> EntryMetadata {
    EntryMetadata {
        path: EntryPath::nfc(path),
        offset,
        size,
        mtime: Mtime::from_unix_seconds(1_700_000_000),
        hash: content_hash(0x5b),
        derived_from: None,
        mime: None,
    }
}

/// The Keyring commitment a commit at `generation` selects (KL-3).
pub(in crate::control) fn keyring(generation: u64) -> KeyringCommitment {
    KeyringCommitment::new(Generation::new(generation), 3, "beef")
        .expect("a lowercase hex digest and a non-zero count are a valid commitment")
}

/// The checkpoint an Index stands at once the head at `generation` is applied.
pub(in crate::control) fn checkpoint(generation: u64) -> IndexCheckpoint {
    IndexCheckpoint {
        master_key_epoch: epoch(2),
        head_generation: Generation::new(generation),
        journal_generation: Generation::new(generation),
        next_commit_slot: Some(format!("minted-{generation}")),
        keyring: keyring(generation),
    }
}

/// The fields of a payload body, for a case that has to change one of them.
pub(in crate::control) fn body_map(payload: &ControlPayload) -> Vec<(Value, Value)> {
    let value: Value =
        ciborium::from_reader(payload.body.as_slice()).expect("a payload body is CBOR");
    match value {
        Value::Map(entries) => entries,
        other => panic!("a payload body is a map, found {other:?}"),
    }
}

/// A payload carrying the fields a case built by hand.
pub(in crate::control) fn with_body_map(
    epoch: MasterKeyEpoch,
    fields: Vec<(Value, Value)>,
) -> ControlPayload {
    let mut bytes = Vec::new();
    ciborium::into_writer(&Value::Map(fields), &mut bytes).expect("a map of values serializes");
    ControlPayload::new(epoch, bytes)
}

/// The value one field of a map carries, for a case that has to change it.
pub(in crate::control) fn field<'a>(fields: &'a mut [(Value, Value)], key: &str) -> &'a mut Value {
    fields
        .iter_mut()
        .find(|(name, _)| name.as_text() == Some(key))
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("the map carries no field {key:?}"))
}

/// The elements one array field of a map carries.
pub(in crate::control) fn array<'a>(
    fields: &'a mut [(Value, Value)],
    key: &str,
) -> &'a mut Vec<Value> {
    match field(fields, key) {
        Value::Array(items) => items,
        other => panic!("{key} is an array, found {other:?}"),
    }
}
