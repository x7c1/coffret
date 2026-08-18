use coffret_model::EntryMetadata;

/// One Entry recovered from a Container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedEntry {
    /// What the entry table recorded about this Entry.
    pub metadata: EntryMetadata,
    /// The Entry's plaintext, verified against `metadata.hash`.
    pub content: Vec<u8>,
}
