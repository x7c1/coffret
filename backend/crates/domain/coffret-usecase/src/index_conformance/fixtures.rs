use std::path::PathBuf;

use coffret_model::{
    ContainerAddition, ContainerId, ContainerKind, ContainerSummary, ContentHash,
    ControlObjectName, EntryLocation, EntryMetadata, EntryPath, Generation, IndexCheckpoint,
    JournalRecord, KeyringCommitment, MasterKeyEpoch, Mtime, ObjectRef, SnapshotContent,
};

use crate::device_state::{
    BatchId, DeviceTime, LocalObservation, Mapping, PendingUpload, RootIdentity, SpoolState,
};

// The values the cases are built out of.
//
// They are deliberately dull — a Container ID is one byte repeated, a hash is
// another — because none of the contract turns on what is in them. What each
// case asserts is where a value ends up, so the values only have to be
// distinguishable from one another at a glance in a failure message.

/// A Container ID whose sixteen bytes are all `seed`.
pub(super) fn container_id(seed: u8) -> ContainerId {
    ContainerId::from_bytes([seed; ContainerId::BYTE_LEN])
}

/// A content hash whose thirty-two bytes are all `seed`.
pub(super) fn content_hash(seed: u8) -> ContentHash {
    ContentHash::from_bytes([seed; ContentHash::BYTE_LEN])
}

/// An Entry Path out of a case's own literal, which every case writes in NFC
/// and so reaches the catalog as it stands (spec: EP-1).
pub(super) fn path(text: &str) -> EntryPath {
    EntryPath::nfc(text)
}

/// The Keyring commitment a commit at `generation` selects (spec: KL-3).
pub(super) fn keyring(generation: u64) -> KeyringCommitment {
    KeyringCommitment::new(Generation::new(generation), 3, "beef")
        .expect("a lowercase hex digest and a non-zero count are a valid commitment")
}

/// The checkpoint an Index stands at once the head at `generation` is applied.
pub(super) fn checkpoint(generation: u64) -> IndexCheckpoint {
    IndexCheckpoint {
        master_key_epoch: MasterKeyEpoch::FIRST,
        head_generation: Generation::new(generation),
        journal_generation: Generation::new(generation),
        next_commit_slot: None,
        keyring: keyring(generation),
    }
}

/// One Container a record adds, holding an Entry at each of `paths`.
///
/// The Entries are laid end to end in the order given, which is what a
/// Container's plaintext stream does (spec: FM-4), so each one gets a distinct
/// offset without a case having to spell one out.
pub(super) fn addition(seed: u8, kind: ContainerKind, paths: &[&str]) -> ContainerAddition {
    let mut entries = Vec::with_capacity(paths.len());
    let mut offset = 0;
    for (position, text) in paths.iter().enumerate() {
        let size = 100 + position as u64;
        entries.push(EntryMetadata {
            path: path(text),
            offset,
            size,
            mtime: Mtime::from_unix_seconds(1_700_000_000 + position as i64),
            hash: content_hash(seed.wrapping_add(position as u8)),
            derived_from: None,
            mime: None,
        });
        offset += size;
    }
    ContainerAddition {
        container: ContainerSummary {
            id: container_id(seed),
            kind,
            ciphertext_hash: content_hash(seed),
            ciphertext_len: offset + 64,
            object_ref: None,
        },
        entries,
    }
}

/// The Journal record that commits `additions` and `removals` as the head at
/// `generation`.
pub(super) fn record(
    generation: u64,
    additions: Vec<ContainerAddition>,
    removals: Vec<ContainerId>,
) -> JournalRecord {
    JournalRecord {
        generation: Generation::new(generation),
        // The first head succeeds nothing; every later one succeeds the head
        // one generation back (spec: FM-13).
        prev: generation.checked_sub(1).map(Generation::new),
        master_key_epoch: MasterKeyEpoch::FIRST,
        keyring: keyring(generation),
        next_commit_slot: None,
        snapshot_slot: None,
        additions,
        removals,
    }
}

/// The Snapshot content of a Library whose head at `generation` holds exactly
/// `containers`, in canonical order.
pub(super) fn snapshot(
    generation: u64,
    containers: Vec<ContainerAddition>,
    adopted_from: Option<ControlObjectName>,
) -> SnapshotContent {
    let mut summaries = Vec::with_capacity(containers.len());
    let mut entries = Vec::new();
    for addition in containers {
        let container_id = addition.container.id;
        summaries.push(addition.container);
        entries.extend(addition.entries.into_iter().map(|entry| EntryLocation {
            container_id,
            entry,
        }));
    }
    SnapshotContent {
        checkpoint: checkpoint(generation),
        adopted_from,
        containers: summaries,
        entries,
    }
    .canonical()
}

/// The name of the ordinary Snapshot checkpointing the head at `generation`
/// (spec: CK-10, FM-12).
pub(super) fn snapshot_name(generation: u64) -> ControlObjectName {
    ControlObjectName::index_snapshot(Generation::new(generation))
}

/// What the two devices at one committed Library state must agree on bit for
/// bit.
///
/// Which checkpoint object a device adopted is its own provenance and differs
/// between a device that replayed records and one that took a later Snapshot,
/// so a comparison of Library-wide content leaves it out (spec: CK-7).
pub(super) fn library_state(mut content: SnapshotContent) -> SnapshotContent {
    content.adopted_from = None;
    content
}

/// What this device saw of a local file it put in place.
pub(super) fn observation(text: &str, size: u64) -> LocalObservation {
    LocalObservation {
        path: path(text),
        size,
        mtime: Mtime::from_unix_seconds(1_700_000_000),
        at: DeviceTime::from_unix_seconds(1_700_000_500),
    }
}

/// A local root mapped to `prefix`, or to the Library root for `None`
/// (spec: EP-9).
///
/// No scan has seen the root, so nothing is recorded about the filesystem under
/// it (spec: EP-12).
pub(super) fn mapping(prefix: Option<&str>, local_root: &str) -> Mapping {
    Mapping {
        prefix: prefix.map(path),
        local_root: PathBuf::from(local_root),
        root_identity: None,
    }
}

/// The same mapping as a scan leaves it, stamped with the filesystem its root
/// stood on (spec: EP-12).
pub(super) fn stamped(prefix: Option<&str>, local_root: &str, identity: &str) -> Mapping {
    Mapping {
        root_identity: Some(RootIdentity::new(identity)),
        ..mapping(prefix, local_root)
    }
}

/// A Container spooled by batch `batch` and not yet committed (spec: OC-2).
///
/// Its spool file is complete, which is what every row a batch can reach the
/// upload or the commit with looks like. An even seed has been uploaded already
/// and an odd one has not, so a suite run covers both the spool that exists only
/// on this disk and the one whose ciphertext is already on Storage waiting for a
/// commit that may never come.
pub(super) fn pending(seed: u8, batch: &str) -> PendingUpload {
    PendingUpload {
        container_id: container_id(seed),
        spool_path: PathBuf::from(format!("/spool/{seed}.cfrt")),
        batch: BatchId::new(batch),
        created_at: DeviceTime::from_unix_seconds(1_700_000_400),
        state: SpoolState::Spooled,
        object_ref: seed
            .is_multiple_of(2)
            .then(|| ObjectRef::new(format!("stored-{seed}"))),
    }
}

/// The same row as a spool step announces it, before its file exists
/// (spec: OC-2).
///
/// No `object_ref`, whatever the seed: a Container is uploaded only out of a
/// finished spool, so a Spooling row never carries one.
pub(super) fn spooling(seed: u8, batch: &str) -> PendingUpload {
    PendingUpload {
        state: SpoolState::Spooling,
        object_ref: None,
        ..pending(seed, batch)
    }
}
