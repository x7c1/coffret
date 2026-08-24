use crate::container_id::ContainerId;
use crate::container_key_status::ContainerKeyStatus;
use crate::key_envelope::KeyEnvelope;

/// One Container the Keyring maps, and what it maps that Container to.
///
/// The pair is the whole of what a Keyring records per Container: which
/// Container, and whether the committed control state holds its envelope or
/// records the key as lost (spec: KL-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyringEntry {
    /// The Container this entry is about.
    pub container_id: ContainerId,
    /// The key status the committed control state records for it.
    pub key: ContainerKeyStatus,
}

impl KeyringEntry {
    /// A Container the Keyring holds an envelope for.
    pub const fn envelope(container_id: ContainerId, envelope: KeyEnvelope) -> Self {
        Self {
            container_id,
            key: ContainerKeyStatus::Envelope(envelope),
        }
    }

    /// A Container the committed control state has no envelope for
    /// (spec: KL-7).
    pub const fn key_lost(container_id: ContainerId) -> Self {
        Self {
            container_id,
            key: ContainerKeyStatus::KeyLost,
        }
    }
}
