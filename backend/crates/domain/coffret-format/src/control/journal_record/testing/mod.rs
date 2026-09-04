//! Helpers shared by the Journal record payload's tests.

use coffret_model::{
    Btime, ContainerAddition, ContainerId, ContainerKind, EntryMetadata, Generation, JournalRecord,
    MasterKeyEpoch,
};

use crate::control::testing::{container_id, entry, epoch, keyring, summary};
use crate::entry_paths::entry_path;

/// The epoch every record these helpers build was committed under.
pub(super) const EPOCH: u64 = 2;

/// The generation the full record below commits at.
pub(super) const GENERATION: u64 = 7;

/// The birth time the one Entry that has one was created at.
pub(super) const BORN: Btime = Btime::from_unix_seconds(1_600_000_000);

pub(super) fn record_epoch() -> MasterKeyEpoch {
    epoch(EPOCH)
}

/// A record with everything a record can carry.
///
/// The additions are handed over in the reverse of Container ID order and the
/// removals likewise, so what a case compares is a record holding them in the
/// order FM-15 fixes rather than in the order a caller happened to have them —
/// which is the whole of what `canonical` is for. One addition caches the
/// provider's handle and one does not, and one carries two Entries so that an
/// entry table of more than one element travels.
pub(super) fn record() -> JournalRecord {
    record_of(
        vec![
            addition(0x40, ContainerKind::Pack),
            addition(0x21, ContainerKind::OneFile),
        ],
        vec![container_id(0x99), container_id(0x11)],
    )
}

/// The record at [`GENERATION`] adding and removing what the two lists name,
/// held in the order FM-15 fixes whichever order they arrive in.
pub(super) fn record_of(
    additions: Vec<ContainerAddition>,
    removals: Vec<ContainerId>,
) -> JournalRecord {
    JournalRecord::canonical(
        Generation::new(GENERATION),
        Some(Generation::new(GENERATION - 1)),
        record_epoch(),
        keyring(4),
        Some("minted-head-8".to_owned()),
        Some("minted-idx-7".to_owned()),
        additions,
        removals,
    )
    .expect("a fixture holds a record a commit could have written")
}

/// The Library's first record: nothing before it, and no slot to persist.
///
/// A name-keyed Storage mints no identifier, so both slots are absent here
/// (CP-2, CP-15) — and generation 0 has no predecessor to state (FM-13).
pub(super) fn first_record() -> JournalRecord {
    first_record_of(vec![addition(0x40, ContainerKind::Pack)])
}

/// The Library's first record, adding what the list names.
pub(super) fn first_record_of(additions: Vec<ContainerAddition>) -> JournalRecord {
    JournalRecord::canonical(
        Generation::FIRST,
        None,
        record_epoch(),
        keyring(0),
        None,
        None,
        additions,
        Vec::new(),
    )
    .expect("a fixture holds the Library's first record")
}

/// One Container a record adds, with an entry table laid end to end (FM-4).
///
/// A Pack's table carries one Entry whose file had a birth time when the
/// Container was written and one whose file had none, so both spellings of the
/// optional field travel (FM-15).
pub(super) fn addition(seed: u8, kind: ContainerKind) -> ContainerAddition {
    addition_of(seed, kind, table(seed, kind))
}

/// One Container a record adds, holding exactly the table handed in.
pub(super) fn addition_of(
    seed: u8,
    kind: ContainerKind,
    entries: Vec<EntryMetadata>,
) -> ContainerAddition {
    ContainerAddition::new(summary(seed, kind), entries)
        .expect("a fixture holds a table that tiles its Container's stream")
}

/// The entry table [`addition`] gives a Container of that seed and kind.
pub(super) fn table(seed: u8, kind: ContainerKind) -> Vec<EntryMetadata> {
    let mut entries = vec![entry(&format!("albums/{seed:02x}/cover.jpg"), 0, 120)];
    if kind == ContainerKind::Pack {
        let mut derived = entry(&format!("albums/{seed:02x}/.thumbs/cover.jpg"), 120, 40);
        derived.btime = Some(BORN);
        derived.mime = Some("image/webp".to_owned());
        derived.derived_from = Some(coffret_model::DerivedFrom {
            container_id: ContainerId::from_bytes([seed; ContainerId::BYTE_LEN]),
            path: entry_path(format!("albums/{seed:02x}/cover.jpg")),
        });
        entries.push(derived);
    }
    entries
}
