//! A Library whose catalog is planted rather than committed.
//!
//! Nothing a case built on it reaches Storage or a file for: it reads, or
//! refuses over, what the catalog holds. So the rows are planted through the
//! same [`Index`] the flows write them through — a Journal record per
//! Container — and the open Library is handed back over that catalog.

use std::sync::Arc;

use coffret_model::{
    CiphertextLenClaim, ContainerAddition, ContainerId, ContainerKind, ContainerSummary,
    ContentHash, DepositedAt, EntryExtent, EntryMetadata, Generation, JournalRecord,
    KeyringCommitment, LibraryId, MasterKey, MasterKeyEpoch, Mtime,
};
use coffret_usecase::{InMemoryIndex, InMemoryStore, Index, LibraryKeys};

use crate::open_library::OpenLibrary;
use crate::testing::{entry_path, local_fs};

/// A Library whose catalog holds the Entries at `paths` and nothing else.
///
/// One Container per call, its kind given, so a case can put a Pack and a
/// one-file Container side by side and ask what a row says about each.
pub(crate) async fn library(planted: &[(u8, ContainerKind, &[&str])]) -> OpenLibrary {
    let index = InMemoryIndex::new();
    for (seed, kind, paths) in planted {
        index
            .apply(record(*seed, *kind, paths))
            .await
            .expect("a record the catalog has never seen replays");
    }
    OpenLibrary {
        store: Arc::new(InMemoryStore::new(64)),
        index: Arc::new(index),
        local_fs: local_fs(),
        trash: local_fs(),
        keys: LibraryKeys::derive(
            &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
            MasterKeyEpoch::FIRST,
        ),
        spool: std::env::temp_dir(),
        parcel_dir: std::env::temp_dir().join("parcels"),
        parcel_len: coffret_format::PARCEL_LEN,
        library_id: LibraryId::from_bytes([0x11; LibraryId::BYTE_LEN]),
        name: None,
        epoch: MasterKeyEpoch::FIRST,
        provider: "s3",
        grant: None,
        births: Default::default(),
        fetch_memory: Default::default(),
        live: Default::default(),
    }
}

/// The generation `number` names, or a panic naming the literal that names
/// none: every number the format carries is bounded (spec: FM-19), a case's
/// own seed included.
fn generation_of(number: u64) -> Generation {
    Generation::new(number)
        .unwrap_or_else(|error| panic!("a case holds a literal generation: {error}"))
}

/// When the Entry at `at` of the Container `seed` names was deposited: a
/// second of its own for each, and none of them its `mtime`.
pub(crate) fn deposited_at(seed: u8, at: usize) -> DepositedAt {
    DepositedAt::from_unix_seconds(1_760_000_000 + i64::from(seed) * 100 + at as i64)
}

/// A record adding one Container of `kind` holding an Entry at each path.
///
/// The generation is the seed, so a case planting two Containers replays two
/// records in the order it named them.
fn record(seed: u8, kind: ContainerKind, paths: &[&str]) -> JournalRecord {
    let generation = generation_of(u64::from(seed));
    let addition = ContainerAddition::new(
        ContainerSummary {
            id: ContainerId::from_bytes([seed; ContainerId::BYTE_LEN]),
            kind,
            ciphertext_hash: ContentHash::from_bytes([seed; ContentHash::BYTE_LEN]),
            ciphertext_len: CiphertextLenClaim::new(1_024)
                .expect("a case's own literal is a length the format admits"),
            object_ref: None,
        },
        paths
            .iter()
            .enumerate()
            .map(|(at, path)| EntryMetadata {
                path: entry_path(*path),
                captured_path: entry_path(*path),
                extent: EntryExtent::new(at as u64 * 100, 100)
                    .expect("a case's own Entries lie inside the address space"),
                mtime: Mtime::from_unix_seconds(1_700_000_000 + at as i64),
                btime: None,
                hash: ContentHash::from_bytes([seed.wrapping_add(at as u8); 32]),
                derived_from: None,
                mime: None,
            })
            .collect(),
        (0..paths.len()).map(|at| deposited_at(seed, at)).collect(),
    )
    .expect("a case's own entry table tiles its Container's stream");

    JournalRecord::new(
        generation,
        seed.checked_sub(1)
            .map(|prev| generation_of(u64::from(prev))),
        MasterKeyEpoch::FIRST,
        KeyringCommitment::new(generation, 3, "beef")
            .expect("a lowercase hex digest and a non-zero count are a valid commitment"),
        None,
        None,
        vec![addition],
        vec![],
    )
    .expect("a case's own record succeeds the head one generation back")
}
