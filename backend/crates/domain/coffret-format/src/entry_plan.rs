use coffret_model::{ContentHash, DerivedFrom, EntryMetadata, EntryPath, Mtime};

/// One Entry declared to the streaming encoder before any of its bytes arrive.
///
/// [`EntrySource`](crate::EntrySource) hands the encoder the content itself and
/// lets it derive the size and the hash. A Pack cannot be written that way — a
/// normal one is around a gigabyte and an oversized singleton can be larger than
/// memory — so the streaming encoder is told the two derived values up front
/// instead, which is what lets it write the entry table before the first byte of
/// the first Entry is read.
///
/// Declaring them is not trusting them. [`ContainerWriter`](crate::ContainerWriter)
/// counts the bytes it is fed for each Entry and hashes them as they pass, and
/// refuses a Container whose bytes are not the ones its table promises — so a
/// file that changed between being surveyed and being read fails the encode
/// rather than reaching Storage under a table that does not describe it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryPlan {
    /// The Library position this Entry occupies.
    pub path: EntryPath,
    /// The file's modification time.
    pub mtime: Mtime,
    /// How many plaintext bytes this Entry is.
    pub size: u64,
    /// BLAKE3-256 of those bytes.
    pub hash: ContentHash,
    /// Set when this Entry holds data derived from another Entry.
    pub derived_from: Option<DerivedFrom>,
    /// The media type of the content, when known.
    pub mime: Option<String>,
}

impl EntryPlan {
    /// An Entry with no optional metadata.
    pub const fn new(path: EntryPath, mtime: Mtime, size: u64, hash: ContentHash) -> Self {
        Self {
            path,
            mtime,
            size,
            hash,
            derived_from: None,
            mime: None,
        }
    }

    /// What the entry table records for this Entry, laid at `offset`.
    ///
    /// The offset is the encoder's to assign — it is where the Entry falls in
    /// the plaintext stream, which depends on every Entry before it — so it is
    /// never a field of the plan.
    pub(crate) fn to_metadata(&self, offset: u64) -> EntryMetadata {
        EntryMetadata {
            path: self.path.clone(),
            offset,
            size: self.size,
            mtime: self.mtime,
            hash: self.hash,
            derived_from: self.derived_from.clone(),
            mime: self.mime.clone(),
        }
    }
}

impl From<&EntryMetadata> for EntryPlan {
    /// The plan that would produce this table row again.
    ///
    /// What a Container already records about an Entry is exactly what a
    /// streaming encoder has to be told to write it a second time, which is the
    /// shape read-modify-replace and repack both work in.
    fn from(entry: &EntryMetadata) -> Self {
        Self {
            path: entry.path.clone(),
            mtime: entry.mtime,
            size: entry.size,
            hash: entry.hash,
            derived_from: entry.derived_from.clone(),
            mime: entry.mime.clone(),
        }
    }
}
