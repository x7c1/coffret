use coffret_model::EntryMetadata;

/// A finished Container: the bytes to upload, the name to upload them under,
/// and the entry table those bytes record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedContainer {
    bytes: Vec<u8>,
    object_name: String,
    entries: Vec<EntryMetadata>,
}

impl EncodedContainer {
    pub(crate) fn new(bytes: Vec<u8>, object_name: String, entries: Vec<EntryMetadata>) -> Self {
        Self {
            bytes,
            object_name,
            entries,
        }
    }

    /// The full object, header first.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The name this object is stored under.
    pub fn object_name(&self) -> &str {
        &self.object_name
    }

    /// The entry table the meta section records, in stream order (spec: FM-9).
    ///
    /// The extents are the ones the layout assigned, which is what makes this
    /// the account of what the object holds rather than a second one: a caller
    /// writing down what it has just encoded — a Journal record's addition, say
    /// (spec: CP-11) — takes the table from here instead of walking its own
    /// inputs again and arriving somewhere else.
    pub fn entries(&self) -> &[EntryMetadata] {
        &self.entries
    }

    /// Takes the bytes, dropping the name and the table.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
