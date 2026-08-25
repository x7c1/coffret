use coffret_model::{ContainerId, ContainerKey, ContainerKind};

use crate::chunk_size::ChunkSize;
use crate::entry_plan::EntryPlan;

/// Everything the streaming encoder needs before the first Entry byte arrives.
///
/// The same four decisions [`EncodeRequest`](crate::EncodeRequest) carries, with
/// the entries declared rather than handed over: a
/// [`ContainerWriter`](crate::ContainerWriter) built from this can write the
/// header and the entry table immediately and then take the content a chunk at
/// a time.
#[derive(Debug, Clone)]
pub struct EncodePlan<'a> {
    /// Identifies the Container and names it on Storage.
    pub container_id: ContainerId,
    /// Whether this Container is one-file or a Pack.
    pub kind: ContainerKind,
    /// The key this Container — and only this Container — is encrypted with.
    pub key: &'a ContainerKey,
    /// Plaintext bytes per chunk, recorded in the header for readers to honor.
    pub chunk_size: ChunkSize,
    /// The entries, in the order they occupy the plaintext stream.
    pub entries: &'a [EntryPlan],
}

impl<'a> EncodePlan<'a> {
    /// A plan using the default chunk size.
    pub fn new(
        container_id: ContainerId,
        kind: ContainerKind,
        key: &'a ContainerKey,
        entries: &'a [EntryPlan],
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
