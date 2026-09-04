use crate::btime::Btime;
use crate::content_hash::ContentHash;
use crate::derived_from::DerivedFrom;
use crate::entry_extent::EntryExtent;
use crate::entry_path::EntryPath;
use crate::mtime::Mtime;

/// What a Container's entry table records about one Entry.
///
/// The `extent` places the Entry against the Container's plaintext stream,
/// which is what lets a reader range-read a single Entry out of a Pack as a
/// step in fetching its Container (PK-16) — the fetch unit stays the whole
/// Container.
///
/// A plain record of values, and it stays one: what could be wrong about an
/// Entry's place in a stream is a condition on the two halves of the extent
/// together, and that is [`EntryExtent`]'s invariant rather than a check this
/// struct makes of its own fields.
///
/// These are the values as of the moment the Container was written, which is
/// why the meta section spells the three of them a later rename could move —
/// `path`, `mtime`, and `btime` — as `original_path`, `original_mtime`, and
/// `original_btime` (FM-9). A Container is immutable, so nothing rewrites them;
/// the Journal and its checkpoint carry the current spelling, which is why a
/// record and a Snapshot spell the same values `path`, `mtime`, and `btime`
/// (FM-15, FM-16). One struct serves both because the values are the same
/// values — only the map key differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMetadata {
    /// The Library position this Entry occupies.
    pub path: EntryPath,
    /// Where this Entry's plaintext lies in the Container's plaintext stream.
    pub extent: EntryExtent,
    /// The file's modification time.
    pub mtime: Mtime,
    /// The file's birth time, where the platform that wrote the Container
    /// reported one.
    pub btime: Option<Btime>,
    /// BLAKE3-256 of this Entry's plaintext.
    pub hash: ContentHash,
    /// Set when this Entry holds data derived from another Entry.
    pub derived_from: Option<DerivedFrom>,
    /// The media type of the content, when known.
    ///
    /// A guess made when the Container was written, and a hint to a reader
    /// rather than a verdict: what may be opened is decided elsewhere (FM-9).
    pub mime: Option<String>,
}
