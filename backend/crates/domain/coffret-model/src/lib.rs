//! Core domain types for coffret Containers and control objects.
//!
//! This crate names the things the rest of the backend talks about — Container
//! identity, kind, keys, entry metadata, and the vocabulary of the control
//! objects that carry a Library's own bookkeeping — and nothing else. It has no
//! third-party dependencies and knows nothing about bytes on the wire: how a
//! Container is serialized, encrypted, and framed lives in `coffret-format`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod container_id;
mod container_key;
mod container_kind;
mod content_hash;
mod control_object_kind;
mod derived_from;
mod entry_metadata;
mod entry_path;
mod error;
mod generation;
mod key_envelope;
mod master_key;
mod master_key_epoch;
mod mtime;
mod replica_position;

pub use container_id::ContainerId;
pub use container_key::ContainerKey;
pub use container_kind::ContainerKind;
pub use content_hash::ContentHash;
pub use control_object_kind::ControlObjectKind;
pub use derived_from::DerivedFrom;
pub use entry_metadata::EntryMetadata;
pub use entry_path::EntryPath;
pub use error::{Error, Result};
pub use generation::Generation;
pub use key_envelope::KeyEnvelope;
pub use master_key::MasterKey;
pub use master_key_epoch::MasterKeyEpoch;
pub use mtime::Mtime;
pub use replica_position::ReplicaPosition;
