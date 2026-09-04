use coffret_format::{
    decode_control_object, decode_keyring, encode_control_object, encode_journal_record,
    encode_keyring, keyring_set_digest, ControlEncodeRequest, Purpose,
};
use coffret_model::{
    ContainerId, ContainerKeyStatus, ControlObjectKind, ControlObjectName, JournalRecord,
    KeyringCommitment, KeyringEntry, KeyringMapping, MasterKeyEpoch,
};

use crate::fetch_conformance::fixtures::keys::purpose_key;
use crate::fetch_conformance::fixtures::objects::{handles, overwrite, replica_name};
use crate::index::Index;
use crate::object_store::ObjectStore;

/// Commits a Keyring generation that records one Container's key as lost, and
/// the head that selects it (spec: KL-3, KL-7, RV-7).
///
/// Written by hand because no flow produces it: losing a key is not something a
/// commit does, and `replicate` refuses to invent a marker for a Container the
/// held mapping has an envelope for — rightly, since that would record a loss
/// the Library never suffered. What a device *would* meet is the state after
/// another device rebuilt the Keyring from the material it had (spec: RV-8), and
/// this is that state: the surviving envelopes carried forward, a marker where
/// the key is gone, and a Journal record selecting the whole tuple.
///
/// The record adds and removes nothing. Nothing in the Library changes but which
/// Keyring generation is committed, which is exactly what a rebuild changes.
pub(crate) async fn lose_key(
    store: &dyn ObjectStore,
    index: &dyn Index,
    container_id: ContainerId,
) {
    let checkpoint = index
        .checkpoint()
        .await
        .expect("reading a checkpoint must succeed")
        .expect("the source device has committed");
    let committed = &checkpoint.keyring();

    let held = read_keyring(store, committed).await;
    let mapping = KeyringMapping::new(
        held.entries()
            .iter()
            .map(|entry| {
                if entry.container_id == container_id {
                    KeyringEntry::key_lost(container_id)
                } else {
                    *entry
                }
            })
            .collect(),
    )
    .expect("a mapping keeps its order when one of its entries changes");
    assert!(
        mapping
            .entries()
            .iter()
            .any(|entry| entry.container_id == container_id
                && entry.key == ContainerKeyStatus::KeyLost),
        "the committed Keyring must have held an envelope for {container_id} to lose",
    );

    let generation = committed
        .generation()
        .next()
        .expect("a generation has a successor");
    let digest = keyring_set_digest(&mapping).expect("a mapping always digests");
    let payload =
        encode_keyring(&mapping, MasterKeyEpoch::FIRST).expect("a mapping always encodes");

    let replicas = committed.replica_count();
    let commitment = KeyringCommitment::new(generation, replicas, &digest)
        .expect("a fresh digest is a valid one");
    for index_of in 0..replicas {
        let name = replica_name(&commitment, index_of);
        overwrite(
            store,
            &name,
            control_bytes(&name, ControlObjectKind::Keyring, &payload),
        )
        .await;
    }

    let head = checkpoint
        .head_generation()
        .next()
        .expect("a generation has a successor");
    let record = JournalRecord::new(
        head,
        Some(checkpoint.head_generation()),
        MasterKeyEpoch::FIRST,
        commitment,
        // `None` because both stores this suite runs against key objects by
        // name, so a slot is re-derived at spend time rather than persisted
        // (spec: CP-15).
        None,
        None,
        Vec::new(),
        Vec::new(),
    )
    .expect("a fixture holds a record succeeding the head it read");
    let name = ControlObjectName::head(head).to_string();
    let payload = encode_journal_record(&record).expect("a record always encodes");
    overwrite(
        store,
        &name,
        control_bytes(&name, ControlObjectKind::Journal, &payload),
    )
    .await;
}

/// The mapping the committed Keyring holds, read from its first replica
/// (spec: KL-1, KL-6).
async fn read_keyring(store: &dyn ObjectStore, commitment: &KeyringCommitment) -> KeyringMapping {
    let name = replica_name(commitment, 0);
    let object = handles(store)
        .await
        .remove(&name)
        .unwrap_or_else(|| panic!("{name:?} must be in Storage"));
    let bytes = store
        .get(&object, None)
        .await
        .unwrap_or_else(|error| panic!("reading {name:?} back must succeed: {error}"))
        .into_bytes()
        .await
        .expect("the stream is as long as it claims");

    let decoded = decode_control_object(&bytes, &name, &purpose_key(Purpose::ControlKeyring))
        .unwrap_or_else(|error| panic!("{name:?} must open as a Keyring: {error}"));
    assert_eq!(decoded.kind, ControlObjectKind::Keyring);
    decode_keyring(&decoded.payload)
        .unwrap_or_else(|error| panic!("{name:?} must decode as FM-17: {error}"))
}

/// Frames one control payload the way its name and kind require (spec: FM-11).
fn control_bytes(
    name: &str,
    kind: ControlObjectKind,
    payload: &coffret_format::ControlPayload,
) -> Vec<u8> {
    let name = ControlObjectName::parse(name).expect("the suite spells valid names");
    encode_control_object(&ControlEncodeRequest::new(
        &name,
        kind,
        &purpose_key(match kind {
            ControlObjectKind::Keyring => Purpose::ControlKeyring,
            ControlObjectKind::Journal => Purpose::ControlJournal,
            ControlObjectKind::IndexSnapshot => Purpose::ControlIndexSnapshot,
            ControlObjectKind::ActivationSnapshot => Purpose::ControlActivationSnapshot,
        }),
        payload,
    ))
    .expect("framing a control object must succeed")
    .bytes()
    .to_vec()
}
