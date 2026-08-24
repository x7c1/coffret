use crate::key_envelope::KeyEnvelope;

/// What the committed Keyring records about one Container's key (spec: KL-7).
///
/// A current Container is mapped either to the envelope that opens it or to the
/// explicit key-lost marker; there is no third state and no absence of one, so
/// a Container is never silently unreadable.
///
/// The marker is a statement about the committed control state alone. It makes
/// no claim about authenticated local key material, which may still restore an
/// envelope later (spec: RV-8), and it does not take the Container out of the
/// current set (spec: KL-17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainerKeyStatus {
    /// The Container Key, wrapped under the Master Key (spec: FM-14).
    Envelope(KeyEnvelope),
    /// No envelope for this Container is reachable from committed control
    /// state (spec: KL-7).
    KeyLost,
}
