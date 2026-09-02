//! Core domain types for coffret Containers and control objects.
//!
//! This crate names the things the rest of the backend talks about — the name
//! a Library carries on Storage ([`LibraryId`], spec: FM-18), Container
//! identity, kind, keys, entry metadata, the vocabulary of the control objects
//! that carry a Library's own bookkeeping, and what a catalog of that Library
//! records about it — [`ContainerSummary`], [`EntryLocation`],
//! [`IndexCheckpoint`] — and nothing else. Its two third-party dependencies are
//! the Unicode composition tables an [`EntryPath`] holds itself to and the
//! overwrite every secret-bearing type here is dropped through. It knows
//! nothing about bytes on the wire: how a Container is serialized, encrypted,
//! and framed lives in `coffret-format`.
//!
//! Which types those are, and what is asked of them, is written out in
//! [`MasterKey`]'s module: it anchors the key hierarchy, so the inventory of
//! everything under it that holds secret bytes lives beside it (spec: DK-7).
//!
//! What a control object carries is part of that vocabulary rather than of any
//! one layer's: [`JournalRecord`] and [`ContainerAddition`] are what a commit
//! writes and a catch-up replays, [`SnapshotContent`] is what an Index Snapshot
//! holds, and [`KeyringMapping`] is what every replica of one Keyring
//! generation carries (spec: CP-11, CK-7, KL-6). `coffret-format` turns them
//! into the bytes FM-15, FM-16, and FM-17 define, and the `Index` port in
//! `coffret-usecase` speaks them; neither owns them.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod btime;
pub use btime::Btime;

mod container_addition;
pub use container_addition::ContainerAddition;

mod container_id;
pub use container_id::ContainerId;

mod container_key;
pub use container_key::ContainerKey;

mod container_key_status;
pub use container_key_status::ContainerKeyStatus;

mod container_kind;
pub use container_kind::ContainerKind;

mod container_summary;
pub use container_summary::ContainerSummary;

mod content_hash;
pub use content_hash::ContentHash;

mod control_object_kind;
pub use control_object_kind::ControlObjectKind;

mod control_object_name;
pub use control_object_name::ControlObjectName;

mod derived_from;
pub use derived_from::DerivedFrom;

mod entry_location;
pub use entry_location::EntryLocation;

mod entry_metadata;
pub use entry_metadata::EntryMetadata;

mod entry_path;
pub use entry_path::EntryPath;

mod error;
pub use error::{Error, Result};

mod generation;
pub use generation::Generation;

mod index_checkpoint;
pub use index_checkpoint::IndexCheckpoint;

mod journal_record;
pub use journal_record::JournalRecord;

mod key_envelope;
pub use key_envelope::KeyEnvelope;

mod keyring_commitment;
pub use keyring_commitment::KeyringCommitment;

mod keyring_entry;
pub use keyring_entry::KeyringEntry;

mod keyring_mapping;
pub use keyring_mapping::KeyringMapping;

mod library_id;
pub use library_id::LibraryId;

// The one hex spelling every identifier and digest in coffret is written in,
// shared by the names that carry one and the commitments that select by one.
mod lowercase_hex;

mod master_key;
pub use master_key::MasterKey;

mod master_key_epoch;
pub use master_key_epoch::MasterKeyEpoch;

mod mtime;
pub use mtime::Mtime;

mod object_ref;
pub use object_ref::ObjectRef;

mod passphrase;
pub use passphrase::Passphrase;

mod replica_position;
pub use replica_position::ReplicaPosition;

mod snapshot_content;
pub use snapshot_content::SnapshotContent;
