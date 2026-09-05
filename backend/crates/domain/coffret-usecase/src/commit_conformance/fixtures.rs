use coffret_format::{wrap_container_key, Purpose, PurposeKey};
use coffret_model::{
    ContainerAddition, ContainerId, ContainerKey, ContainerKind, ContainerSummary, ContentHash,
    EntryMetadata, EntryPath, KeyEnvelope, MasterKey, MasterKeyEpoch, Mtime,
};

use crate::ciphertext_len_claims::ciphertext_len;
use crate::commit::{CommitPolicy, CommitRequest, ControlKeys, PreparedAddition, PreparedBatch};
use crate::entry_extents::entry_extent;
use crate::entry_paths::entry_path;
use crate::index::Index;
use crate::object_store::ObjectStore;

// The values the cases are built out of.
//
// The Containers are dull on purpose — an ID is one byte repeated, a hash is
// another — because nothing the commit protocol does depends on what is inside
// one. The keys are not dull in the same way: they are real, derived from a
// real Master Key, because the cases read every control object back and a
// fixture that faked the crypto would prove nothing about what a device would
// find on Storage.

/// The Master Key the whole suite works under.
///
/// Fixed rather than generated, so that the two devices in a case are two
/// devices of one Library. Every purpose key in a case comes from this one.
pub(super) fn master_key() -> MasterKey {
    MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN])
}

/// The control-object keys of the Library's first epoch.
pub(super) fn control_keys() -> ControlKeys {
    ControlKeys::derive(&master_key(), MasterKeyEpoch::FIRST)
}

/// The purpose key one kind of control object is sealed under (spec: KD-4).
///
/// The cases derive their own rather than borrowing the flow's, so that what
/// they check a stored object against is the rule and not the code under test.
pub(super) fn purpose_key(purpose: Purpose) -> PurposeKey {
    PurposeKey::derive(&master_key(), purpose)
}

/// A Container ID whose sixteen bytes are all `seed`.
pub(super) fn container_id(seed: u8) -> ContainerId {
    ContainerId::from_bytes([seed; ContainerId::BYTE_LEN])
}

/// An Entry Path out of a case's own literal, which every case writes in NFC
/// and so reaches the catalog as it stands (spec: EP-1).
pub(super) fn path(text: &str) -> EntryPath {
    entry_path(text)
}

/// The envelope the Keyring maps one Container to (spec: FM-14, KL-7).
///
/// A real wrap under a real Container Key, so the mapping a case reads back
/// carries 72 bytes that would actually open something.
pub(super) fn envelope(seed: u8) -> KeyEnvelope {
    wrap_container_key(
        &purpose_key(Purpose::ContainerWrap),
        &container_id(seed),
        &ContainerKey::from_bytes([seed; ContainerKey::BYTE_LEN]),
    )
    .expect("wrapping a Container Key under the container-wrap key must succeed")
}

/// One Container of a batch, holding an Entry at each of `paths`.
///
/// The Entries are laid end to end in the order given, which is what a
/// Container's plaintext stream does (spec: FM-4), so each gets a distinct
/// offset without a case spelling one out.
pub(super) fn prepared(seed: u8, kind: ContainerKind, paths: &[&str]) -> PreparedAddition {
    let mut entries = Vec::with_capacity(paths.len());
    let mut offset = 0;
    for (position, text) in paths.iter().enumerate() {
        let size = 100 + position as u64;
        entries.push(EntryMetadata {
            path: path(text),
            extent: entry_extent(offset, size),
            mtime: Mtime::from_unix_seconds(1_700_000_000 + position as i64),
            btime: None,
            hash: ContentHash::from_bytes(
                [seed.wrapping_add(position as u8); ContentHash::BYTE_LEN],
            ),
            derived_from: None,
            mime: None,
        });
        offset += size;
    }
    let container = ContainerSummary {
        id: container_id(seed),
        kind,
        ciphertext_hash: ContentHash::from_bytes([seed; ContentHash::BYTE_LEN]),
        ciphertext_len: ciphertext_len(offset + 64),
        // A cache and never evidence of membership: a record that carries none
        // leaves a reader to re-derive the handle from a listing (spec: FM-15).
        // The cases put their Containers on Storage themselves, so there is no
        // handle to carry here.
        object_ref: None,
    };
    PreparedAddition::new(
        ContainerAddition::new(container, entries)
            .expect("a fixture holds a table that tiles its Container's stream"),
        envelope(seed),
    )
}

/// A policy that keeps a case's Library small and its failures quick.
///
/// Two replicas rather than three, so a case that drops one still has a
/// complete-looking set to fail against; a checkpoint threshold high enough
/// that no case writes a Snapshot unless it asked to.
pub(super) fn policy() -> CommitPolicy {
    CommitPolicy::default()
        .with_replica_count(2)
        .with_checkpoint_threshold(NEVER_CHECKPOINT)
}

/// A threshold no case reaches by committing.
pub(super) const NEVER_CHECKPOINT: u64 = 1_000;

/// A commit of `batch` against one device's store and catalog, under the
/// suite's policy.
pub(super) fn request<'a>(
    store: &'a dyn ObjectStore,
    index: &'a dyn Index,
    keys: &'a ControlKeys,
    batch: PreparedBatch,
) -> CommitRequest<'a> {
    CommitRequest::new(store, index, keys, batch).with_policy(policy())
}
