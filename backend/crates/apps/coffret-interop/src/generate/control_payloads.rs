//! The payloads the fixture set's control objects carry.
//!
//! Each is here because it pins something two implementations can disagree
//! about: a record with additions carrying entry tables and a removal (FM-15), a
//! Snapshot whose Entries interleave across several Containers so that the two
//! canonical orders are both exercised, the activation Snapshot's two extra
//! fields (FM-16, MR-2), and a Keyring mapping holding both of the things a
//! Keyring holds for a Container (FM-17). Every array is built out of the
//! canonical order on purpose, so a set whose writer left the order alone fails
//! the exchange.

use coffret_format::{keyring_set_digest, IndexSnapshotPayload, SnapshotActivation};
use coffret_model::{
    Btime, ContainerAddition, ContainerId, ContainerKind, ContainerSummary, ContentHash,
    DerivedFrom, EntryLocation, EntryMetadata, EntryPath, Generation, IndexCheckpoint,
    JournalRecord, KeyEnvelope, KeyringCommitment, KeyringEntry, KeyringMapping, MasterKeyEpoch,
    Mtime, ObjectRef, SnapshotContent,
};

use super::{EPOCH, KEYRING_REPLICA_GENERATION};

/// The head the fixture Journal record commits at.
pub(super) const JOURNAL_GENERATION: u64 = 7;

/// The head the fixture ordinary Snapshot checkpoints (CK-10).
pub(super) const SNAPSHOT_GENERATION: u64 = 4;

/// The head the fixture activation Snapshot took (MR-2).
pub(super) const ACTIVATION_GENERATION: u64 = 2;

/// The mapping the `keyring-replica` fixture carries (FM-17).
///
/// Two Containers open through an envelope and one is recorded key-lost, so
/// both of the things a Keyring holds for a Container travel (KL-7) — and the
/// entries are built out of Container ID order, like every other array in the
/// set.
///
/// The envelopes are filler rather than wrappings of the set's real Container
/// Keys: FM-17 carries an envelope as an opaque byte string of the length FM-14
/// gives it, and whether one unwraps is what the `key-envelope` fixture is for.
pub(super) fn keyring_mapping() -> KeyringMapping {
    KeyringMapping::new(vec![
        KeyringEntry::envelope(container_id(0x40), envelope(0x40)),
        KeyringEntry::key_lost(container_id(0x99)),
        KeyringEntry::envelope(container_id(0x21), envelope(0x21)),
    ])
}

/// The digest the generated Keyring replica set carries in its name (FM-12).
///
/// Computed from the mapping rather than chosen: the name a replica is stored
/// under has to be the one FM-17's digest gives it, or the reader that
/// recomputes the digest from the payload it opened would find a name no
/// mapping produced.
pub(super) fn set_digest() -> String {
    keyring_set_digest(&keyring_mapping()).expect("a fixture mapping serializes")
}

/// A Key Envelope whose seventy-two bytes are all `seed`.
fn envelope(seed: u8) -> KeyEnvelope {
    KeyEnvelope::from_bytes([seed; KeyEnvelope::BYTE_LEN])
}

/// The record the `journal` fixture carries (FM-15).
pub(super) fn journal_record() -> JournalRecord {
    JournalRecord {
        generation: Generation::new(JOURNAL_GENERATION),
        prev: Some(Generation::new(JOURNAL_GENERATION - 1)),
        master_key_epoch: epoch(),
        keyring: keyring(),
        // The minted form a Storage that mints identifiers leaves in a head
        // (CP-2), for both slots the head reserves (CK-10).
        next_commit_slot: Some("minted-head-8".to_owned()),
        snapshot_slot: Some("minted-idx-7".to_owned()),
        additions: vec![
            addition(0x40, ContainerKind::Pack),
            addition(0x21, ContainerKind::OneFile),
        ],
        removals: vec![container_id(0x99), container_id(0x11)],
    }
}

/// The Snapshot the `index-snapshot` fixture carries (FM-16).
pub(super) fn ordinary_snapshot() -> IndexSnapshotPayload {
    IndexSnapshotPayload::ordinary(library(SNAPSHOT_GENERATION))
}

/// The Snapshot the `activation-snapshot` fixture carries (FM-16, MR-2).
///
/// It carries no `activation_slot`: a Storage that keys objects by name mints
/// nothing, so what an activation from one records is the head it fenced and
/// nothing else (CP-2, CP-15). The exchange therefore covers both an activation
/// Snapshot without a slot token and, in the Journal record above, slots that
/// carry one.
pub(super) fn activation_snapshot() -> IndexSnapshotPayload {
    IndexSnapshotPayload::activating(
        library(ACTIVATION_GENERATION),
        SnapshotActivation {
            base_head_generation: Generation::new(ACTIVATION_GENERATION - 1),
            activation_slot: None,
        },
    )
}

/// A Library of three Containers whose Entries interleave across them.
///
/// Interleaving is the point: `entries` is in Entry Path order across the whole
/// Library (EP-3) rather than grouped by Container, so a reader that grouped
/// them lands somewhere else.
fn library(generation: u64) -> SnapshotContent {
    let containers = vec![
        summary(0x40, ContainerKind::Pack),
        summary(0x21, ContainerKind::OneFile),
        summary(0x33, ContainerKind::Pack),
    ];
    // One Entry records a birth time and the rest record none, so a Snapshot
    // carries both spellings of the optional field (FM-16).
    let mut born = located(0x40, "albums/spring/a.jpg", 0, 100);
    born.entry.btime = Some(Btime::from_unix_seconds(1_600_000_000));
    let entries = vec![
        located(0x33, "photos/2019/b.jpg", 0, 90),
        born,
        located(0x21, "books/atlas/page-001.png", 0, 200),
        located(0x40, "photos/2019/a.jpg", 100, 80),
    ];
    SnapshotContent {
        checkpoint: IndexCheckpoint {
            master_key_epoch: epoch(),
            head_generation: Generation::new(generation),
            journal_generation: Generation::new(generation),
            next_commit_slot: Some(format!("minted-head-{}", generation + 1)),
            keyring: keyring(),
        },
        // Which checkpoint an Index adopted is device state and no Snapshot
        // carries it (CK-7), so a set that stated one would be stating something
        // no object can hold.
        adopted_from: None,
        containers,
        entries,
    }
}

/// One Container a record adds, with an entry table laid end to end (FM-4).
///
/// A Pack carries a second Entry that is derived from the first, whose `mtime`
/// predates 1970 and which records a birth time, so a record's entry table
/// exercises the optional fields and the signed times the meta section's does
/// (FM-9, FM-15). The first Entry records no birth time, so the absent
/// spelling travels beside the present one.
fn addition(seed: u8, kind: ContainerKind) -> ContainerAddition {
    let label = format!("{seed:02x}");
    let mut entries = vec![entry(&format!("albums/{label}/cover.jpg"), 0, 120)];
    if kind == ContainerKind::Pack {
        let mut derived = entry(&format!("albums/{label}/.thumbs/cover.jpg"), 120, 40);
        derived.mtime = Mtime::from_unix_seconds(-2_208_988_800);
        derived.btime = Some(Btime::from_unix_seconds(-2_208_988_800));
        derived.mime = Some("image/webp".to_owned());
        derived.derived_from = Some(DerivedFrom {
            container_id: container_id(seed),
            path: EntryPath::nfc(format!("albums/{label}/cover.jpg")),
        });
        entries.push(derived);
    }
    ContainerAddition {
        container: summary(seed, kind),
        entries,
    }
}

/// What a payload records about one Container.
///
/// An even seed caches the provider's handle for the object and an odd one does
/// not, so both spellings of the optional field travel (CP-11).
fn summary(seed: u8, kind: ContainerKind) -> ContainerSummary {
    ContainerSummary {
        id: container_id(seed),
        kind,
        ciphertext_hash: ContentHash::from_bytes([seed; ContentHash::BYTE_LEN]),
        ciphertext_len: 4096 + u64::from(seed),
        object_ref: seed
            .is_multiple_of(2)
            .then(|| ObjectRef::new(format!("stored-{seed}"))),
    }
}

fn located(seed: u8, path: &str, offset: u64, size: u64) -> EntryLocation {
    EntryLocation {
        container_id: container_id(seed),
        entry: entry(path, offset, size),
    }
}

fn entry(path: &str, offset: u64, size: u64) -> EntryMetadata {
    EntryMetadata {
        path: EntryPath::nfc(path),
        offset,
        size,
        mtime: Mtime::from_unix_seconds(1_700_000_000),
        btime: None,
        hash: ContentHash::from_bytes([0x5b; ContentHash::BYTE_LEN]),
        derived_from: None,
        mime: None,
    }
}

fn container_id(seed: u8) -> ContainerId {
    ContainerId::from_bytes([seed; ContainerId::BYTE_LEN])
}

/// The Keyring tuple every control payload in the set commits to (CP-10, KL-3).
///
/// It is the tuple of the replica the set also carries — down to the digest of
/// that replica's own mapping (FM-17) — so the record, the two Snapshots, and
/// that replica all name one replica set rather than three.
fn keyring() -> KeyringCommitment {
    KeyringCommitment::new(
        Generation::new(KEYRING_REPLICA_GENERATION),
        3,
        &set_digest(),
    )
    .expect("the set digest is lowercase hex and the count is non-zero")
}

fn epoch() -> MasterKeyEpoch {
    MasterKeyEpoch::new(EPOCH).expect("the epoch is valid")
}
