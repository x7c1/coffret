use coffret_model::{DerivedFrom, EntryPath, Mtime};

/// One Entry handed to the encoder.
///
/// The encoder derives `offset`, `size`, and `hash` itself from the position and
/// content given here, so those three cannot disagree with the bytes actually
/// stored.
#[derive(Debug, Clone)]
pub struct EntrySource<'a> {
    /// The Library position this Entry occupies.
    pub path: EntryPath,
    /// The file's modification time.
    pub mtime: Mtime,
    /// The Entry's plaintext.
    pub content: &'a [u8],
    /// Set when this Entry holds data derived from another Entry.
    pub derived_from: Option<DerivedFrom>,
    /// The media type of the content, when known.
    pub mime: Option<String>,
}

impl<'a> EntrySource<'a> {
    /// An Entry with no optional metadata.
    pub fn new(path: EntryPath, mtime: Mtime, content: &'a [u8]) -> Self {
        Self {
            path,
            mtime,
            content,
            derived_from: None,
            mime: None,
        }
    }
}
