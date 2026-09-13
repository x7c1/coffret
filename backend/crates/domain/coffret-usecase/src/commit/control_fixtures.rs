//! The small Library the commit module's own cases are driven over.
//!
//! Sealing a control object takes a Master Key, a Keyring commitment, and an
//! encoder, and none of that is what any case here is about — so it is written
//! once, and a case says only what makes its own Library different from the
//! plain one.

use coffret_format::{
    encode_control_object, keyring_set_digest, ControlEncodeRequest, ControlPayload,
};
use coffret_model::{
    ControlObjectKind, ControlObjectName, Generation, JournalRecord, KeyringCommitment,
    KeyringMapping, MasterKey, MasterKeyEpoch, ObjectRef,
};

use crate::byte_stream::ByteStream;
use crate::commit::control_keys::ControlKeys;
use crate::generations::generation;
use crate::in_memory_store::InMemoryStore;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// Objects small enough that a listing page never matters to what is asserted.
pub(super) const PAGE_SIZE: usize = 8;

/// One attempt and no waiting.
///
/// What is on trial in these cases is the first answer, and a policy that
/// retried would only ask for it five more times — and make a case that counts
/// calls count a policy's attempts instead of a flow's reads.
pub(super) fn once() -> RetryPolicy {
    RetryPolicy::default().with_attempts(1)
}

/// The Master Key these Libraries work under.
pub(super) fn control_keys() -> ControlKeys {
    ControlKeys::derive(
        &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
        MasterKeyEpoch::FIRST,
    )
}

/// The Keyring tuple every head in them names (spec: CP-10).
///
/// The same empty mapping at every generation: no case here reads a Keyring,
/// and a commitment that names one is all a record and a Snapshot have to
/// carry.
pub(super) fn commitment() -> KeyringCommitment {
    let digest = keyring_set_digest(&KeyringMapping::default()).expect("a mapping always digests");
    KeyringCommitment::new(Generation::FIRST, 1, &digest)
        .expect("one replica of a real digest is a commitment")
}

/// The Journal record committed at one generation, adding and removing nothing.
///
/// Empty on purpose: what these cases are about is which objects a flow reads
/// and in what order, and a record carrying Containers would only make the
/// fixture longer.
pub(super) fn record_at(head: Generation) -> JournalRecord {
    JournalRecord::new(
        head,
        head.get().checked_sub(1).map(generation),
        MasterKeyEpoch::FIRST,
        commitment(),
        None,
        None,
        Vec::new(),
        Vec::new(),
    )
    .expect("a fixture holds a record succeeding the head one generation back")
}

/// Seals one payload as the control object at `name` and stores it.
pub(super) async fn store_control(
    store: &InMemoryStore,
    keys: &ControlKeys,
    name: &ControlObjectName,
    kind: ControlObjectKind,
    payload: &ControlPayload,
) -> ObjectRef {
    let object = encode_control_object(&ControlEncodeRequest::new(
        name,
        kind,
        keys.of_kind(kind),
        payload,
    ))
    .expect("sealing a control object under a real key must succeed");
    store
        .put(&name.to_string(), ByteStream::from(object.bytes().to_vec()))
        .await
        .expect("storing a control object must succeed")
}
