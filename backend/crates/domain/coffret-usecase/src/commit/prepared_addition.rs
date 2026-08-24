use coffret_model::{ContainerAddition, KeyEnvelope};

/// One Container of a batch, ready to be committed.
///
/// The two halves travel together and end up in two different objects, which is
/// the whole reason this type exists. What the Container holds goes into the
/// Journal record (spec: CP-11); the key that opens it goes into the Keyring
/// candidate and never into the record (spec: CP-11, KL-7). A caller that held
/// them apart could commit a record naming a Container the committed Keyring has
/// no envelope for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedAddition {
    /// What the Journal record records about the Container (spec: CP-11).
    pub addition: ContainerAddition,
    /// The envelope the next Keyring generation maps it to (spec: KL-7).
    pub envelope: KeyEnvelope,
}

impl PreparedAddition {
    /// Pairs what a record says about a Container with the key that opens it.
    pub const fn new(addition: ContainerAddition, envelope: KeyEnvelope) -> Self {
        Self { addition, envelope }
    }
}
