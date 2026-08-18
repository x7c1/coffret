use coffret_model::{ContainerId, ContainerKind};

use crate::chunk_size::ChunkSize;
use crate::decoded_entry::DecodedEntry;

/// An opened Container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedContainer {
    /// The Container ID from the header.
    pub container_id: ContainerId,
    /// The chunk size the object was written with.
    pub chunk_size: ChunkSize,
    /// Whether this Container is one-file or a Pack.
    pub kind: ContainerKind,
    /// The entries, in plaintext stream order.
    pub entries: Vec<DecodedEntry>,
}
