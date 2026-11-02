use std::fmt;

/// The kinds of fixture a set carries, by the manifest list each is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureKind {
    /// A Container, under `containers`.
    Container,
    /// A control object, under `control_objects`.
    ControlObject,
    /// A Key Envelope, under `key_envelopes`.
    KeyEnvelope,
    /// A stored Master Key, under `stored_master_keys`.
    StoredMasterKey,
    /// A byte string no reader accepts as a stored Master Key, under
    /// `refused_stored_master_keys`.
    RefusedStoredMasterKey,
    /// A Recovery Code, under `recovery_codes`.
    RecoveryCode,
}

impl fmt::Display for FixtureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Container => "Container",
            Self::ControlObject => "control-object",
            Self::KeyEnvelope => "Key Envelope",
            Self::StoredMasterKey => "stored Master Key",
            Self::RefusedStoredMasterKey => "refused stored Master Key",
            Self::RecoveryCode => "Recovery Code",
        })
    }
}
