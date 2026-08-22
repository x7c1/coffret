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
pub use container_id::ContainerId;

mod container_key;
pub use container_key::ContainerKey;

mod container_kind;
pub use container_kind::ContainerKind;

mod content_hash;
pub use content_hash::ContentHash;

mod control_object_kind;
pub use control_object_kind::ControlObjectKind;

mod control_object_name;
pub use control_object_name::ControlObjectName;

mod derived_from;
pub use derived_from::DerivedFrom;

mod entry_metadata;
pub use entry_metadata::EntryMetadata;

mod entry_path;
pub use entry_path::EntryPath;

mod error;
pub use error::{Error, Result};

mod generation;
pub use generation::Generation;

mod key_envelope;
pub use key_envelope::KeyEnvelope;

mod master_key;
pub use master_key::MasterKey;

mod master_key_epoch;
pub use master_key_epoch::MasterKeyEpoch;

mod mtime;
pub use mtime::Mtime;

mod replica_position;
pub use replica_position::ReplicaPosition;
