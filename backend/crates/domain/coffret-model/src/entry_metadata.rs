use crate::btime::Btime;
use crate::content_hash::ContentHash;
use crate::derived_from::DerivedFrom;
use crate::entry_extent::EntryExtent;
use crate::entry_path::EntryPath;
use crate::mtime::Mtime;

/// What a Container's entry table records about one Entry.
///
/// The `extent` places the Entry against the Container's plaintext stream,
/// which is what lets a reader take a single Entry out of a Pack by reading
/// only the parcels it overlaps (spec: PK-16, PK-19) — the fetch unit is the
/// parcel, never the Entry.
///
/// A plain record of values, and it stays one: what could be wrong about an
/// Entry's place in a stream is a condition on the two halves of the extent
/// together, and that is [`EntryExtent`]'s invariant rather than a check this
/// struct makes of its own fields.
///
/// These are the values as captured when the Container was written, which is
/// why the meta section names the three of them the Library may later hold
/// differently — `path`, `mtime`, and `btime` — as `captured_path`,
/// `captured_mtime`, and `captured_btime` (spec: FM-9). A Container is
/// immutable, so nothing rewrites them; the Journal and its checkpoint carry
/// the current spelling, which is why a record and a Snapshot name the same
/// values `path`, `mtime`, and `btime` (spec: FM-15, FM-16). One struct serves
/// both because the values are the same values — only the field differs.
///
/// The one exception is the name. The catalog keeps both the Entry Path the
/// Library holds now (`path`) and the one the owning Container's meta section
/// recorded (`captured_path`), so that every device can name both without
/// opening the Container; a record and a Snapshot carry the captured one
/// wherever it differs from the current one (spec: FM-15, FM-16). Read out of
/// a meta section, or laid out by a writer, the two are the same path: the
/// name a Container records is the one the Entry had when it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMetadata {
    /// The Library position this Entry occupies.
    pub path: EntryPath,
    /// The Entry Path the owning Container's meta section records for this
    /// Entry: its name as of the moment that Container was written
    /// (spec: FM-9).
    ///
    /// The name it was captured under rather than where it stands. Equal to
    /// `path` until a rename gives the Entry a new path, and never changed by
    /// one, because the Container it describes is never rewritten.
    pub captured_path: EntryPath,
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
    /// rather than a verdict: what may be opened is decided elsewhere
    /// (spec: FM-9).
    pub mime: Option<String>,
}

impl EntryMetadata {
    /// Whether `recorded` — what a Container's own entry table records at this
    /// Entry's entry number — is this Entry, on everything a rename leaves
    /// alone: the name it was captured under, where its plaintext lies, and
    /// what that plaintext hashes to (spec: FM-9, FM-15).
    ///
    /// The current path is left out because a rename changes it and no
    /// Container is rewritten to follow, so a catalog row and its meta entry
    /// differ there by design. The rest of the row is left out too: the
    /// modification and birth times are values the Library may later hold
    /// differently from what the Container captured (spec: FM-9), and the
    /// three compared here are what says which bytes the Entry is.
    pub fn is_recorded_as(&self, recorded: &EntryMetadata) -> bool {
        self.captured_path == recorded.captured_path
            && self.extent == recorded.extent
            && self.hash == recorded.hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(text: &str) -> EntryPath {
        EntryPath::parse(text).expect("a fixture holds a literal path")
    }

    fn recorded() -> EntryMetadata {
        EntryMetadata {
            path: path("albums/a.jpg"),
            captured_path: path("albums/a.jpg"),
            extent: EntryExtent::new(8, 4).expect("a small extent"),
            mtime: Mtime::from_unix_seconds(1_700_000_000),
            btime: None,
            hash: ContentHash::from_bytes([0x11; 32]),
            derived_from: None,
            mime: None,
        }
    }

    // FM-9, FM-15: a renamed row is still the Entry its Container records,
    // and one whose captured path, extent, or hash differs is not.
    #[test]
    fn a_row_is_recorded_as_its_meta_entry_whatever_its_current_path() {
        let meta = recorded();
        let renamed = EntryMetadata {
            path: path("books/a.jpg"),
            ..recorded()
        };
        assert!(renamed.is_recorded_as(&meta));

        let elsewhere = EntryMetadata {
            captured_path: path("albums/b.jpg"),
            ..recorded()
        };
        let other_extent = EntryMetadata {
            extent: EntryExtent::new(12, 4).expect("a small extent"),
            ..recorded()
        };
        let other_content = EntryMetadata {
            hash: ContentHash::from_bytes([0x22; 32]),
            ..recorded()
        };
        for row in [elsewhere, other_extent, other_content] {
            assert!(!row.is_recorded_as(&meta), "{row:?}");
        }
    }
}
