//! Helpers shared by the Journal record payload's tests.

use coffret_model::{
    ContainerAddition, ContainerId, ContainerKind, Generation, JournalRecord, MasterKeyEpoch,
};

use crate::control::testing::{container_id, entry, epoch, keyring, summary};

/// The epoch every record these helpers build was committed under.
pub(super) const EPOCH: u64 = 2;

/// The generation the full record below commits at.
pub(super) const GENERATION: u64 = 7;

pub(super) fn record_epoch() -> MasterKeyEpoch {
    epoch(EPOCH)
}

/// A record with everything a record can carry.
///
/// The additions are handed over in the reverse of Container ID order and the
/// removals likewise, so a case comparing bytes is comparing what the encoder
/// ordered rather than what a caller happened to hold (FM-15). One addition
/// caches the provider's handle and one does not, and one carries two Entries
/// so that an entry table of more than one element travels.
pub(super) fn record() -> JournalRecord {
    JournalRecord {
        generation: Generation::new(GENERATION),
        prev: Some(Generation::new(GENERATION - 1)),
        master_key_epoch: record_epoch(),
        keyring: keyring(4),
        next_commit_slot: Some("minted-head-8".to_owned()),
        snapshot_slot: Some("minted-idx-7".to_owned()),
        additions: vec![
            addition(0x40, ContainerKind::Pack),
            addition(0x21, ContainerKind::OneFile),
        ],
        removals: vec![container_id(0x99), container_id(0x11)],
    }
}

/// The Library's first record: nothing before it, and no slot to persist.
///
/// A name-keyed Storage mints no identifier, so both slots are absent here
/// (CP-2, CP-15) — and generation 0 has no predecessor to state (FM-13).
pub(super) fn first_record() -> JournalRecord {
    JournalRecord {
        generation: Generation::FIRST,
        prev: None,
        master_key_epoch: record_epoch(),
        keyring: keyring(0),
        next_commit_slot: None,
        snapshot_slot: None,
        additions: vec![addition(0x40, ContainerKind::Pack)],
        removals: Vec::new(),
    }
}

/// One Container a record adds, with an entry table laid end to end (FM-4).
pub(super) fn addition(seed: u8, kind: ContainerKind) -> ContainerAddition {
    let mut entries = vec![entry(&format!("albums/{seed:02x}/cover.jpg"), 0, 120)];
    if kind == ContainerKind::Pack {
        let mut derived = entry(&format!("albums/{seed:02x}/.thumbs/cover.jpg"), 120, 40);
        derived.mime = Some("image/webp".to_owned());
        derived.derived_from = Some(coffret_model::DerivedFrom {
            container_id: ContainerId::from_bytes([seed; ContainerId::BYTE_LEN]),
            path: coffret_model::EntryPath::new(format!("albums/{seed:02x}/cover.jpg")),
        });
        entries.push(derived);
    }
    ContainerAddition {
        container: summary(seed, kind),
        entries,
    }
}
