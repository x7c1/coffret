//! The payloads the fixture set's control objects carry.
//!
//! Each is here because it pins something two implementations can disagree
//! about: a record with additions carrying entry tables, removals, and renames
//! naming Entries by Container and entry number (spec: FM-15), a Snapshot whose
//! Entries interleave across several Containers so that the two canonical
//! orders are both exercised and each carries its entry number, the activation
//! Snapshot's two extra fields (spec: FM-16, MR-2), the Library's name a record
//! and a Snapshot carry where they carry one (spec: FM-15, FM-16), and a
//! Keyring's key table
//! holding both of the things a Keyring holds for a Container (spec: FM-17).
//! Every array is handed over out of the canonical order on purpose and put in
//! it by the aggregate that carries it, so a set whose writer left the order
//! alone fails the exchange.
//!
//! The literals here are the generator's own, so a value one of them cannot
//! build is a mistake in this module: each unwrap says so, the way the `entry`
//! helper below does.

use coffret_format::{keyring_set_digest, IndexSnapshotPayload, SnapshotActivation};
use coffret_model::{
    Btime, CiphertextLenClaim, ContainerAddition, ContainerId, ContainerKind, ContainerSummary,
    ContentHash, DepositedAt, DerivedFrom, EntryExtent, EntryLocation, EntryMetadata, EntryNumber,
    EntryRename, IndexCheckpoint, JournalRecord, KeyEnvelope, KeyTable, KeyringCommitment,
    KeyringElement, LibraryName, MasterKeyEpoch, Mtime, ObjectRef, SnapshotContent,
};

use super::{entry_path, generation, EPOCH, KEYRING_REPLICA_GENERATION};

/// The head the fixture Journal record commits at.
pub(super) const JOURNAL_GENERATION: u64 = 7;

/// The head the fixture ordinary Snapshot checkpoints (spec: CK-10).
pub(super) const SNAPSHOT_GENERATION: u64 = 4;

/// The head the fixture activation Snapshot took (spec: MR-2).
pub(super) const ACTIVATION_GENERATION: u64 = 2;

/// The key table the `keyring-replica` fixture carries (spec: FM-17).
///
/// Two Containers open through an envelope and one is recorded key-lost, so
/// both of the things a Keyring holds for a Container travel (spec: KL-7) —
/// and the elements are built out of Container ID order, like every other
/// array in the set.
///
/// The envelopes are filler rather than wrappings of the set's real Container
/// Keys: FM-17 carries an envelope as an opaque byte string of the length FM-14
/// gives it, and whether one unwraps is what the `key-envelope` fixture is for.
pub(super) fn key_table() -> KeyTable {
    KeyTable::canonical(vec![
        KeyringElement::envelope(container_id(0x40), envelope(0x40)),
        KeyringElement::key_lost(container_id(0x99)),
        KeyringElement::envelope(container_id(0x21), envelope(0x21)),
    ])
    .expect("the generator's own key table names each Container once")
}

/// The digest the generated Keyring replica set carries in its name
/// (spec: FM-12).
///
/// Computed from the key table rather than chosen: the name a replica is stored
/// under has to be the one FM-17's digest gives it, or the reader that
/// recomputes the digest from the payload it opened would find a name no
/// key table produced.
pub(super) fn set_digest() -> String {
    keyring_set_digest(&key_table()).expect("a fixture key table serializes")
}

/// A Key Envelope whose seventy-two bytes are all `seed`.
fn envelope(seed: u8) -> KeyEnvelope {
    KeyEnvelope::from_bytes([seed; KeyEnvelope::BYTE_LEN])
}

/// The record the `journal` fixture carries (spec: FM-15).
///
/// It renames three Entries of Containers an earlier record added — two in one
/// Pack, so the order by entry number travels beside the order by Container —
/// and one new path is not ASCII, so both sides carry the same NFC bytes. It
/// names the Library too, with a name that is not ASCII either.
pub(super) fn journal_record() -> JournalRecord {
    JournalRecord::canonical(
        generation(JOURNAL_GENERATION),
        Some(generation(JOURNAL_GENERATION - 1)),
        epoch(),
        keyring(),
        // The minted form a Storage that mints identifiers leaves in a head
        // (spec: CP-2), for both slots the head reserves (spec: CK-10).
        Some("minted-head-8".to_owned()),
        Some("minted-idx-7".to_owned()),
        vec![
            addition(0x40, ContainerKind::Pack),
            addition(0x21, ContainerKind::OneFile),
        ],
        vec![container_id(0x99), container_id(0x11)],
    )
    .and_then(|record| {
        record.with_renames(vec![
            rename(0x05, 0, "albums/renamed/first.jpg"),
            rename(0x05, 3, "albums/renamed/caf\u{e9}.jpg"),
            rename(0x30, 1, "notes/kept.txt"),
        ])
    })
    .map(|record| record.with_library_name(Some(library_name())))
    .expect("the generator's own record is one a commit could have written")
}

/// The name the `journal` and `index-snapshot` fixtures give the Library
/// (spec: FM-15, FM-16).
fn library_name() -> LibraryName {
    LibraryName::parse("Photos 写真").expect("the generator's own name is one")
}

/// The rename of entry `number` of the Container `seed` names to `path`.
fn rename(seed: u8, number: u32, path: &str) -> EntryRename {
    EntryRename::new(container_id(seed), entry_number(number), entry_path(path))
        .expect("the generator's own renames stay outside the reserved namespace")
}

fn entry_number(number: u32) -> EntryNumber {
    EntryNumber::new(u64::from(number)).expect("every u32 is an entry number")
}

/// The Snapshot the `index-snapshot` fixture carries (spec: FM-16), naming the
/// Library; the activation one below names nothing, so both spellings of the
/// optional field travel.
pub(super) fn ordinary_snapshot() -> IndexSnapshotPayload {
    IndexSnapshotPayload::ordinary(
        library(SNAPSHOT_GENERATION).with_library_name(Some(library_name())),
    )
}

/// The Snapshot the `activation-snapshot` fixture carries (spec: FM-16, MR-2).
///
/// It carries no `activation_slot`: a Storage that keys objects by name mints
/// nothing, so what an activation from one records is the head it fenced and
/// nothing else (spec: CP-2, CP-15). The exchange therefore covers both an
/// activation Snapshot without a slot token and, in the Journal record above,
/// slots that carry one.
pub(super) fn activation_snapshot() -> IndexSnapshotPayload {
    IndexSnapshotPayload::activating(
        library(ACTIVATION_GENERATION),
        SnapshotActivation {
            base_head_generation: generation(ACTIVATION_GENERATION - 1),
            activation_slot: None,
        },
    )
}

/// A Library of three Containers whose Entries interleave across them.
///
/// Interleaving is the point: `entries` is in Entry Path order across the whole
/// Library (spec: EP-3) rather than grouped by Container, so a reader that
/// grouped them lands somewhere else. Each Entry carries its number in its
/// Container's table, and the Pack `0x40` holds two, so a number other than 0
/// travels.
fn library(number: u64) -> SnapshotContent {
    let containers = vec![
        summary(0x40, ContainerKind::Pack),
        summary(0x21, ContainerKind::OneFile),
        summary(0x33, ContainerKind::Pack),
    ];
    // One Entry records a birth time and the rest record none, so a Snapshot
    // carries both spellings of the optional field (spec: FM-16). The same
    // Entry stands at a path other than the one its Container recorded, so the
    // Snapshot carries `captured_path` for it and leaves it out for the rest.
    let mut born = located(0x40, 0, "albums/spring/a.jpg", 0, 100);
    born.entry.btime = Some(Btime::from_unix_seconds(1_600_000_000));
    born.entry.captured_path = entry_path("imports/a.jpg");
    let entries = vec![
        located(0x33, 0, "photos/2019/b.jpg", 0, 90),
        born,
        located(0x21, 0, "books/atlas/page-001.png", 0, 200),
        located(0x40, 1, "photos/2019/a.jpg", 100, 80),
    ];
    SnapshotContent::canonical(
        IndexCheckpoint::at_head(
            epoch(),
            generation(number),
            Some(format!("minted-head-{}", number + 1)),
            keyring(),
        ),
        // Which checkpoint an Index adopted is device state and no Snapshot
        // carries it (spec: CK-7), so a set that stated one would be stating
        // something no object can hold.
        None,
        containers,
        entries,
    )
    .expect("the generator's own Library is one an Index could stand at")
}

/// One Container a record adds, with an entry table laid end to end
/// (spec: FM-4).
///
/// A Pack carries a second Entry that is derived from the first, whose `mtime`
/// predates 1970 and which records a birth time, so a record's entry table
/// exercises the optional fields and the signed times the meta section's does
/// (spec: FM-9, FM-15). The first Entry records no birth time, so the absent
/// spelling travels beside the present one; it also stands at a path other
/// than the one its Container recorded, so `captured_path` travels for it and
/// is left out for the derived Entry.
fn addition(seed: u8, kind: ContainerKind) -> ContainerAddition {
    let label = format!("{seed:02x}");
    let mut cover = entry(&format!("albums/{label}/cover.jpg"), 0, 120);
    cover.captured_path = entry_path(format!("imports/{label}/cover.jpg"));
    let mut entries = vec![cover];
    if kind == ContainerKind::Pack {
        let mut derived = entry(&format!("albums/{label}/.thumbs/cover.jpg"), 120, 40);
        derived.mtime = Mtime::from_unix_seconds(-2_208_988_800);
        derived.btime = Some(Btime::from_unix_seconds(-2_208_988_800));
        derived.mime = Some("image/webp".to_owned());
        derived.derived_from = Some(DerivedFrom {
            container_id: container_id(seed),
            path: entry_path(format!("albums/{label}/cover.jpg")),
        });
        entries.push(derived);
    }
    let deposit_times = (0..entries.len())
        .map(|position| deposited_at(seed, position as u32))
        .collect();
    ContainerAddition::new(summary(seed, kind), entries, deposit_times)
        .expect("the generator's own entry table tiles its Container's stream")
}

/// When entry `number` of the Container `seed` names was deposited
/// (spec: FM-15, FM-16): a second of its own for every Entry, and none of them
/// an `mtime` the generator writes, so a reader that took one for another is
/// caught.
fn deposited_at(seed: u8, number: u32) -> DepositedAt {
    DepositedAt::from_unix_seconds(1_760_000_000 + i64::from(seed) * 1_000 + i64::from(number))
}

/// What a payload records about one Container.
///
/// An even seed caches the provider's handle for the object and an odd one does
/// not, so both spellings of the optional field travel (spec: CP-11).
fn summary(seed: u8, kind: ContainerKind) -> ContainerSummary {
    ContainerSummary {
        id: container_id(seed),
        kind,
        ciphertext_hash: ContentHash::from_bytes([seed; ContentHash::BYTE_LEN]),
        ciphertext_len: CiphertextLenClaim::new(4096 + u64::from(seed))
            .expect("the generator's own literal is a length the format admits"),
        object_ref: seed
            .is_multiple_of(2)
            .then(|| ObjectRef::new(format!("stored-{seed}"))),
    }
}

fn located(seed: u8, number: u32, path: &str, offset: u64, size: u64) -> EntryLocation {
    EntryLocation {
        container_id: container_id(seed),
        entry_number: entry_number(number),
        entry: entry(path, offset, size),
        deposited_at: deposited_at(seed, number),
    }
}

fn entry(path: &str, offset: u64, size: u64) -> EntryMetadata {
    EntryMetadata {
        path: entry_path(path),
        captured_path: entry_path(path),
        extent: EntryExtent::new(offset, size)
            .expect("the generator's own literals lay an Entry inside the address space"),
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

/// The Keyring tuple every control payload in the set commits to
/// (spec: CP-10, KL-3).
///
/// It is the tuple of the replica the set also carries — down to the digest of
/// that replica's own key table (spec: FM-17) — so the record, the two
/// Snapshots, and that replica all name one replica set rather than three.
fn keyring() -> KeyringCommitment {
    KeyringCommitment::new(generation(KEYRING_REPLICA_GENERATION), 3, &set_digest())
        .expect("the set digest is lowercase hex and the count is non-zero")
}

fn epoch() -> MasterKeyEpoch {
    MasterKeyEpoch::new(EPOCH).expect("the epoch is valid")
}
