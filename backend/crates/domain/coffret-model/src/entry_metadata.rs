use crate::content_hash::ContentHash;
use crate::derived_from::DerivedFrom;
use crate::entry_path::EntryPath;
use crate::mtime::Mtime;

/// What a Container's entry table records about one Entry.
///
/// `offset` and `size` place the Entry against the Container's plaintext
/// stream, which is what lets a reader range-read a single Entry out of a Pack
/// as a step in fetching its Container (PK-16) — the fetch unit stays the
/// whole Container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMetadata {
    /// The Library position this Entry occupies.
    pub path: EntryPath,
    /// Byte offset of this Entry's plaintext in the Container's plaintext stream.
    pub offset: u64,
    /// Length of this Entry's plaintext in bytes.
    pub size: u64,
    /// The file's modification time.
    pub mtime: Mtime,
    /// BLAKE3-256 of this Entry's plaintext.
    pub hash: ContentHash,
    /// Set when this Entry holds data derived from another Entry.
    pub derived_from: Option<DerivedFrom>,
    /// The media type of the content, when known.
    pub mime: Option<String>,
}
