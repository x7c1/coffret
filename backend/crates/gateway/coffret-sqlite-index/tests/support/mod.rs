//! Values the adapter's own cases are built out of, and the file-level helpers
//! two of them need to look at an Index file from outside the port.
//!
//! The conformance suite has its own, kept private to it: these cases are about
//! the file rather than about the contract, so they build the smallest content
//! that puts something in every Library-wide table — including a derived-from
//! reference and a mime type, which are the nullable columns a round trip
//! through the file could otherwise silently drop.
//!
//! [`Scratch`] and the raw-SQLite readers beside it are shared by the schema
//! and refused-Index suites, both of which build files behind the adapter's
//! back and then have to check what is actually written into them.

// Each integration test file that declares `mod support;` compiles this module
// on its own, as part of that file's own binary — so an item only one of the
// two suites calls is unused as far as the other's compilation is concerned.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use coffret_model::{
    Btime, ContainerId, ContainerKind, ContainerSummary, ContentHash, ControlObjectName,
    DerivedFrom, EntryLocation, EntryMetadata, EntryPath, Generation, IndexCheckpoint,
    KeyringCommitment, MasterKeyEpoch, Mtime, ObjectRef,
};
use coffret_usecase::{ContainerAddition, JournalRecord, SnapshotContent};

/// The Entry Path a literal spells, or a panic naming the one that spells none.
///
/// Every Entry Path is built by reading text (spec: EP-1, EP-2), a fixture's
/// included; the unwrap lives here so that a mistyped literal is reported once,
/// as the mistake in the fixture that it is.
pub fn entry_path(text: impl Into<String>) -> EntryPath {
    EntryPath::parse(text)
        .unwrap_or_else(|error| panic!("a fixture holds a literal Entry Path: {error}"))
}

/// A Container ID whose sixteen bytes are all `seed`.
pub fn container_id(seed: u8) -> ContainerId {
    ContainerId::from_bytes([seed; ContainerId::BYTE_LEN])
}

/// The checkpoint of the head at `generation`.
pub fn checkpoint(generation: u64) -> IndexCheckpoint {
    IndexCheckpoint {
        master_key_epoch: MasterKeyEpoch::FIRST,
        head_generation: Generation::new(generation),
        journal_generation: Generation::new(generation),
        // A minted identifier, the shape a Storage that mints them leaves in a
        // head (spec: CP-2) — and the one that has to survive the file.
        next_commit_slot: Some(format!("minted-{generation}")),
        keyring: KeyringCommitment::new(Generation::new(generation), 3, "beef")
            .expect("a lowercase hex digest and a non-zero count are a valid commitment"),
    }
}

/// One Pack holding two Entries, the second derived from the first.
///
/// The paths carry the seed, so two Packs never claim one Entry Path — which
/// they may not, at any committed state (spec: EP-5).
pub fn addition(seed: u8) -> ContainerAddition {
    let original = entry_path(format!("albums/{seed}.jpg"));
    ContainerAddition {
        container: ContainerSummary {
            id: container_id(seed),
            kind: ContainerKind::Pack,
            ciphertext_hash: ContentHash::from_bytes([seed; ContentHash::BYTE_LEN]),
            ciphertext_len: 164,
            object_ref: Some(ObjectRef::new(format!("stored-{seed}"))),
        },
        entries: vec![
            EntryMetadata {
                path: original.clone(),
                offset: 0,
                size: 100,
                mtime: Mtime::from_unix_seconds(1_700_000_000),
                // The platform that wrote this Container reported a birth time
                // and the one below it did not, so the column is exercised full
                // and NULL (spec: FM-9, FM-15).
                btime: Some(Btime::from_unix_seconds(-86_400)),
                hash: ContentHash::from_bytes([seed; ContentHash::BYTE_LEN]),
                derived_from: None,
                mime: Some("image/jpeg".to_owned()),
            },
            EntryMetadata {
                path: entry_path(format!("albums/{seed}.thumb.jpg")),
                offset: 100,
                size: 64,
                mtime: Mtime::from_unix_seconds(1_700_000_001),
                btime: None,
                hash: ContentHash::from_bytes([seed.wrapping_add(1); ContentHash::BYTE_LEN]),
                derived_from: Some(DerivedFrom {
                    container_id: container_id(seed),
                    path: original,
                }),
                mime: Some("image/jpeg".to_owned()),
            },
        ],
    }
}

/// The Snapshot of a Library whose head at `generation` holds one Pack.
pub fn snapshot(generation: u64) -> SnapshotContent {
    let addition = addition(1);
    let container_id = addition.container.id;
    SnapshotContent {
        checkpoint: checkpoint(generation),
        adopted_from: Some(ControlObjectName::index_snapshot(Generation::new(
            generation,
        ))),
        containers: vec![addition.container],
        entries: addition
            .entries
            .into_iter()
            .map(|entry| EntryLocation {
                container_id,
                entry,
            })
            .collect(),
    }
    .canonical()
}

/// A record that adds one more Pack as the head at `generation`.
pub fn record(generation: u64) -> JournalRecord {
    let checkpoint = checkpoint(generation);
    // The record and the checkpoint it reaches are one state, so the record
    // carries the same slot the checkpoint records (spec: CK-1, CK-2).
    let seed = u8::try_from(generation % 100).expect("a value under 100 fits in a byte");
    JournalRecord {
        generation: checkpoint.head_generation,
        prev: generation.checked_sub(1).map(Generation::new),
        master_key_epoch: checkpoint.master_key_epoch,
        keyring: checkpoint.keyring,
        next_commit_slot: checkpoint.next_commit_slot,
        // The other slot a head reserves, in the same minted form
        // (spec: CK-10).
        snapshot_slot: Some(format!("minted-idx-{generation}")),
        // Never seed 1: that is the Pack a restored Snapshot brings, and one
        // Container is added once.
        additions: vec![addition(seed + 2)],
        removals: vec![],
    }
}

/// A path inside a directory that lives as long as the case.
pub struct Scratch {
    directory: tempfile::TempDir,
}

impl Scratch {
    pub fn new() -> Self {
        Self {
            directory: tempfile::tempdir().expect("a temporary directory must be creatable"),
        }
    }

    pub fn file(&self) -> PathBuf {
        self.directory.path().join("index.sqlite")
    }

    /// The directory itself, for a case that has to name it rather than the
    /// file inside it.
    pub fn path(&self) -> &Path {
        self.directory.path()
    }
}

/// The version an Index file is stamped with, read past the adapter.
pub fn stamp_of(file: &Path) -> i64 {
    rusqlite::Connection::open(file)
        .expect("the Index file must open")
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("SQLite always answers with a stamp")
}

/// How many rows one table holds, read past the adapter.
///
/// A refused file is one the port will not open at all, so the only way to ask
/// what is left in it is to ask SQLite.
pub fn rows_in(file: &Path, table: &str) -> i64 {
    rusqlite::Connection::open(file)
        .expect("the Index file must open")
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("counting a table this build laid out must succeed")
}
