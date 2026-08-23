use coffret_model::{ContainerSummary, EntryMetadata};

/// One Container a Journal record adds, with everything it holds.
///
/// A record carries each new Container's ciphertext hash, its kind, and its
/// entry table, in the meta section's own vocabulary, which is exactly what
/// lets a device replaying the record rebuild its Index without opening a
/// single Container (spec: CP-11, CK-9, RV-5). The Container's authenticated
/// meta section remains the authority on what it holds; this is the copy the
/// record travels with.
///
/// No Key Envelope ever rides here: which Containers are current is the
/// Journal's business, and the committed Keyring is the only Storage home of
/// the keys that open them (spec: CP-11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerAddition {
    /// What the record records about the Container itself.
    pub container: ContainerSummary,
    /// The Container's entry table (spec: FM-9).
    pub entries: Vec<EntryMetadata>,
}
