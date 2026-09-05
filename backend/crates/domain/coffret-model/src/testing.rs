//! The one place this crate's own tests turn literals into the values the
//! control aggregates are built out of.
//!
//! Every leaf here is built the way production code builds it — an Entry Path
//! is parsed, an extent is placed against the address space, a Keyring
//! commitment is checked against its digest's spelling — so a fixture that
//! cannot be built is reported once, here, as the mistake in the fixture that
//! it is rather than as a refusal from the constructor under test.
//!
//! Nothing here is a shortcut past a rule: the five aggregates are always built
//! through their own constructors, and what these functions supply is only the
//! parts.

use crate::btime::Btime;
use crate::ciphertext_len_claim::CiphertextLenClaim;
use crate::container_id::ContainerId;
use crate::container_kind::ContainerKind;
use crate::container_summary::ContainerSummary;
use crate::content_hash::ContentHash;
use crate::entry_extent::EntryExtent;
use crate::entry_location::EntryLocation;
use crate::entry_metadata::EntryMetadata;
use crate::entry_path::EntryPath;
use crate::generation::Generation;
use crate::key_envelope::KeyEnvelope;
use crate::keyring_commitment::KeyringCommitment;
use crate::keyring_entry::KeyringEntry;
use crate::master_key_epoch::MasterKeyEpoch;
use crate::mtime::Mtime;

/// The generation `number` names, or a panic naming the literal that names
/// none.
pub(crate) fn generation(number: u64) -> Generation {
    Generation::new(number)
        .unwrap_or_else(|error| panic!("a fixture holds a literal generation: {error}"))
}

/// The ciphertext length `len` claims, or a panic naming the literal that
/// claims none.
pub(crate) fn ciphertext_len(len: u64) -> CiphertextLenClaim {
    CiphertextLenClaim::new(len)
        .unwrap_or_else(|error| panic!("a fixture holds a literal ciphertext length: {error}"))
}

/// The Entry Path `text` spells, or a panic naming the literal that spells
/// none.
pub(crate) fn entry_path(text: &str) -> EntryPath {
    EntryPath::parse(text)
        .unwrap_or_else(|error| panic!("a fixture holds a literal Entry Path: {error}"))
}

/// The extent `offset` and `size` place an Entry at, or a panic naming the pair
/// that places none.
pub(crate) fn entry_extent(offset: u64, size: u64) -> EntryExtent {
    EntryExtent::new(offset, size)
        .unwrap_or_else(|error| panic!("a fixture holds a literal extent: {error}"))
}

/// The Container ID whose sixteen bytes are all `seed`, so that ordering by ID
/// is ordering by the number a case wrote down.
pub(crate) fn container_id(seed: u8) -> ContainerId {
    ContainerId::from_bytes([seed; ContainerId::BYTE_LEN])
}

/// What the Index records about the Container `seed` names.
pub(crate) fn container_summary(seed: u8) -> ContainerSummary {
    ContainerSummary {
        id: container_id(seed),
        kind: ContainerKind::OneFile,
        ciphertext_hash: ContentHash::from_bytes([seed; ContentHash::BYTE_LEN]),
        ciphertext_len: ciphertext_len(4096),
        object_ref: None,
    }
}

/// One Entry at `path`, occupying `size` bytes of the stream from `offset`.
pub(crate) fn entry(path: &str, offset: u64, size: u64) -> EntryMetadata {
    EntryMetadata {
        path: entry_path(path),
        extent: entry_extent(offset, size),
        mtime: Mtime::from_unix_seconds(1_700_000_000),
        btime: Some(Btime::from_unix_seconds(1_600_000_000)),
        hash: ContentHash::from_bytes([7; ContentHash::BYTE_LEN]),
        derived_from: None,
        mime: None,
    }
}

/// An entry table laid out by `(offset, size)`, each Entry at a path of its
/// own, for cases about how a table tiles rather than about what it holds.
pub(crate) fn table(layout: &[(u64, u64)]) -> Vec<EntryMetadata> {
    layout
        .iter()
        .enumerate()
        .map(|(index, (offset, size))| entry(&format!("albums/{index}.jpg"), *offset, *size))
        .collect()
}

/// An entry table spelled out in full, for cases about the paths in it.
pub(crate) fn table_at(entries: &[(&str, u64, u64)]) -> Vec<EntryMetadata> {
    entries
        .iter()
        .map(|(path, offset, size)| entry(path, *offset, *size))
        .collect()
}

/// Where the Entry at `path` lives, held by the Container `seed` names.
pub(crate) fn entry_location(seed: u8, path: &str, offset: u64, size: u64) -> EntryLocation {
    EntryLocation {
        container_id: container_id(seed),
        entry: entry(path, offset, size),
    }
}

/// The Keyring replica set a fixture's commit selects.
pub(crate) fn keyring_commitment() -> KeyringCommitment {
    KeyringCommitment::new(generation(3), 2, &"ab".repeat(32))
        .unwrap_or_else(|error| panic!("a fixture holds a literal commitment: {error}"))
}

/// The epoch a fixture's control state stands in.
pub(crate) fn master_key_epoch() -> MasterKeyEpoch {
    MasterKeyEpoch::new(1)
        .unwrap_or_else(|error| panic!("a fixture holds a literal epoch: {error}"))
}

/// The Keyring's entry for the Container `seed` names.
pub(crate) fn keyring_entry(seed: u8) -> KeyringEntry {
    KeyringEntry::envelope(
        container_id(seed),
        KeyEnvelope::from_bytes([seed; KeyEnvelope::BYTE_LEN]),
    )
}
