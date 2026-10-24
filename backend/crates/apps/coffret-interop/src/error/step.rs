use std::fmt;

/// A step of writing or opening a fixture, or of a dump, that a lower layer
/// can refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Drawing a Master Key from the entropy source.
    DrawingMasterKey,
    /// Drawing a Container ID from the entropy source.
    DrawingContainerId,
    /// Drawing a Container Key from the entropy source.
    DrawingContainerKey,
    /// Building a value the generator writes the set with: a chunk size, an
    /// epoch, a replica position, an object name.
    StatingGeneratorValue,
    /// Encoding a Container.
    EncodingContainer,
    /// Encoding a control payload.
    EncodingPayload,
    /// Sealing a control object.
    EncodingControlObject,
    /// Wrapping a Container Key into a Key Envelope.
    WrappingContainerKey,
    /// Protecting a Master Key under a Passphrase.
    ProtectingMasterKey,
    /// Opening a Container.
    OpeningContainer,
    /// Laying an Entry out in the plaintext stream from the contents the
    /// manifest states.
    LayingOutEntry,
    /// Opening a control object.
    OpeningControlObject,
    /// Reading a Journal record out of its payload.
    ReadingJournalRecord,
    /// Reading an Index Snapshot out of its payload.
    ReadingIndexSnapshot,
    /// Reading a Keyring's key table out of its payload.
    ReadingKeyring,
    /// Parsing the name a Keyring replica is stored under.
    ParsingReplicaName,
    /// Digesting a key table.
    DigestingKeyTable,
    /// Reading a Key Envelope's bytes.
    ReadingKeyEnvelope,
    /// Unwrapping a Container Key from its envelope.
    UnwrappingContainerKey,
    /// Reading a stored Master Key's form.
    ReadingStoredMasterKey,
    /// Unlocking a stored Master Key with a Passphrase.
    UnlockingMasterKey,
    /// Parsing a Recovery Code.
    ParsingRecoveryCode,
    /// Reading a Container's header.
    ReadingContainerHeader,
    /// Opening a Container's meta section.
    OpeningMetaSection,
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::DrawingMasterKey => "drawing a Master Key",
            Self::DrawingContainerId => "drawing a Container ID",
            Self::DrawingContainerKey => "drawing a Container Key",
            Self::StatingGeneratorValue => "stating a value the set is written with",
            Self::EncodingContainer => "encoding the Container",
            Self::EncodingPayload => "encoding the control payload",
            Self::EncodingControlObject => "encoding the control object",
            Self::WrappingContainerKey => "wrapping a Container Key",
            Self::ProtectingMasterKey => "protecting the Master Key under the Passphrase",
            Self::OpeningContainer => "opening the Container",
            Self::LayingOutEntry => {
                "laying the Entry out in the address space from the manifest's own contents"
            }
            Self::OpeningControlObject => "opening the control object",
            Self::ReadingJournalRecord => "reading the Journal record",
            Self::ReadingIndexSnapshot => "reading the Index Snapshot",
            Self::ReadingKeyring => "reading the Keyring",
            Self::ParsingReplicaName => "parsing the replica's name",
            Self::DigestingKeyTable => "digesting the key table",
            Self::ReadingKeyEnvelope => "reading the Key Envelope",
            Self::UnwrappingContainerKey => "unwrapping the Container Key",
            Self::ReadingStoredMasterKey => "reading the stored Master Key",
            Self::UnlockingMasterKey => "unlocking with the manifest's Passphrase",
            Self::ParsingRecoveryCode => "parsing the Recovery Code",
            Self::ReadingContainerHeader => "reading the Container header",
            Self::OpeningMetaSection => "opening the meta section",
        })
    }
}
