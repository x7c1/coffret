use coffret_model::{ContainerId, ContainerKey, ContainerKind};

use crate::chunk_size::ChunkSize;
use crate::entry_source::EntrySource;

/// Everything the encoder needs to lay out one Container.
#[derive(Debug, Clone)]
pub struct EncodeRequest<'a> {
    /// Identifies the Container and names it on Storage.
    pub container_id: ContainerId,
    /// Whether this Container is one-file or a Pack.
    pub kind: ContainerKind,
    /// The key this Container — and only this Container — is encrypted with.
    pub key: &'a ContainerKey,
    /// Plaintext bytes per chunk, recorded in the header for readers to honor.
    pub chunk_size: ChunkSize,
    /// The entries, in the order they occupy the plaintext stream.
    pub entries: &'a [EntrySource<'a>],
}

impl<'a> EncodeRequest<'a> {
    /// A request using the default chunk size.
    pub fn new(
        container_id: ContainerId,
        kind: ContainerKind,
        key: &'a ContainerKey,
        entries: &'a [EntrySource<'a>],
    ) -> Self {
        Self {
            container_id,
            kind,
            key,
            chunk_size: ChunkSize::DEFAULT,
            entries,
        }
    }
}
