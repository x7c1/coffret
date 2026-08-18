//! Core domain types for coffret Containers.
//!
//! This crate names the things the rest of the backend talks about — Container
//! identity, kind, keys, and entry metadata — and nothing else. It has no
//! third-party dependencies and knows nothing about bytes on the wire: how a
//! Container is serialized, encrypted, and framed lives in `coffret-format`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod container_id;
mod container_key;
mod container_kind;
mod content_hash;
mod derived_from;
mod entry_metadata;
mod entry_path;
mod error;
mod mtime;

pub use container_id::ContainerId;
pub use container_key::ContainerKey;
pub use container_kind::ContainerKind;
pub use content_hash::ContentHash;
pub use derived_from::DerivedFrom;
pub use entry_metadata::EntryMetadata;
pub use entry_path::EntryPath;
pub use error::{Error, Result};
pub use mtime::Mtime;
