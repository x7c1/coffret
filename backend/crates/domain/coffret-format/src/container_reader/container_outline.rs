use std::ops::Range;

use coffret_model::{ContainerId, ContainerKey, ContainerKind, EntryMetadata, EntryPath};

use crate::aead::Cipher;
use crate::chunk_size::ChunkSize;
use crate::container_reader::chunk_layout::ChunkLayout;
use crate::container_reader::chunk_run::ChunkRun;
use crate::error::{Error, Result};
use crate::header::Header;
use crate::meta;
use crate::nonce;

/// What a Container's front says about the rest of it.
///
/// A Container's whole shape is settled by its 32-byte header and the meta
/// section behind it (spec: FM-2, FM-9): the entry table places every Entry in
/// the plaintext stream, and the chunk size and the stream's padded length place
/// every chunk in the object. So a reader that has those bytes — a few kilobytes
/// at the front of an object that may be gigabytes — knows exactly which bytes
/// to ask Storage for next, and asks for those.
///
/// The outline carries no key. It is *opened* with one, because the meta section
/// is an AEAD message under the Container Key like everything else in the object
/// (spec: FM-1, FM-8), and what it holds afterwards is the entry table, which
/// travels no further than the reader that asked for it.
///
/// The header those few kilobytes are counted out from is unauthenticated
/// plaintext, and the count is what the next read is sized by. So the declared
/// length is held against [`Header::MAX_META_LEN`] as the header is parsed,
/// which is before [`prefix_len`](Self::prefix_len) has a number to hand back
/// and before [`open`](Self::open) has a slice to take.
///
/// ```
/// use coffret_format::{ChunkRunReader, ContainerOutline, EncodeRequest, EntrySource};
/// use coffret_model::{ContainerKey, ContainerKind, EntryPath, Mtime};
///
/// # fn main() -> coffret_format::Result<()> {
/// let key = ContainerKey::from_bytes([0x42; ContainerKey::BYTE_LEN]);
/// let content = b"the page a reader asked for";
/// let entries = [EntrySource::new(
///     EntryPath::parse("books/atlas/003.jpg")?,
///     Mtime::from_unix_seconds(1_700_000_000),
///     content,
/// )];
/// let object = coffret_format::encode(&EncodeRequest::new(
///     coffret_format::generate_container_id()?,
///     ContainerKind::Pack,
///     &key,
///     &entries,
/// ))?;
/// let object = object.bytes();
///
/// // In real use these two slices are two range reads: the first tells the
/// // reader how long the second has to be.
/// let prefix_len = ContainerOutline::prefix_len(&object[..32])? as usize;
/// let outline = ContainerOutline::open(&object[..prefix_len], &key)?;
///
/// let entry = &outline.entries()[0];
/// let run = outline.chunks_covering(entry.extent.range())?;
/// let asked = run.ciphertext();
/// assert!(asked.end - asked.start <= outline.object_len());
///
/// let mut reader = ChunkRunReader::begin(&outline, &key, &run);
/// let mut plaintext = Vec::new();
/// reader.read(&object[asked.start as usize..asked.end as usize], &mut plaintext)?;
/// reader.finish()?;
///
/// // The run begins at a chunk boundary, which is at or before the Entry, so
/// // the Entry's own bytes start this far into what the run opened.
/// let start = usize::try_from(entry.extent.offset() - run.plaintext_start())
///     .expect("this Entry begins inside what one run opened");
/// let len = usize::try_from(entry.extent.size()).expect("and it fits in memory");
/// assert_eq!(&plaintext[start..start + len], content);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerOutline {
    header: Header,
    kind: ContainerKind,
    pad_len: u64,
    entries: Vec<EntryMetadata>,
    layout: ChunkLayout,
}

impl ContainerOutline {
    /// How many bytes of an object's front an outline is read from.
    ///
    /// The answer is in the header, so a reader asks for [`Header::LEN`] bytes,
    /// puts them here, and asks for exactly what this says next. It is a
    /// plaintext question and takes no key (spec: FM-2) — which is why the
    /// answer is bounded: a header declaring more than
    /// [`Header::MAX_META_LEN`] yields no length at all, so no range request is
    /// ever aimed at one.
    pub fn prefix_len(front: &[u8]) -> Result<u64> {
        let header = Header::parse(front)?;
        Ok(Header::LEN as u64 + u64::from(header.meta_len))
    }

    /// Reads the header and opens the meta section from an object's front.
    ///
    /// `prefix` has to be at least [`prefix_len`](Self::prefix_len) bytes;
    /// anything beyond that is ignored, so the whole object serves as its own
    /// prefix. The header is validated on its plaintext bytes before the key is
    /// used at all, exactly as [`decode`](crate::decode()) does it.
    pub fn open(prefix: &[u8], key: &ContainerKey) -> Result<Self> {
        let header = Header::parse(prefix)?;

        // The associated data is the header exactly as it appears in the object
        // (spec: FM-8).
        let associated_data = &prefix[..Header::LEN];
        let meta_len = usize::try_from(header.meta_len).map_err(|_| Error::Truncated)?;
        let meta_section = prefix
            .get(Header::LEN..Header::LEN + meta_len)
            .ok_or(Error::Truncated)?;

        let cipher = Cipher::new(key.as_bytes());
        let meta = meta::decode(&cipher.open(&nonce::meta(), associated_data, meta_section)?)?;
        let layout = ChunkLayout::of(&header, meta.plaintext_len()?)?;

        Ok(Self {
            header,
            kind: meta.kind,
            pad_len: meta.pad_len,
            entries: meta.entries,
            layout,
        })
    }

    /// The Container ID the header carries (spec: FM-2).
    pub fn container_id(&self) -> ContainerId {
        self.header.container_id
    }

    /// The 32 header bytes, which are the associated data of every AEAD message
    /// in the object (spec: FM-8).
    ///
    /// Rebuilt from the parsed fields rather than kept as a slice, which is the
    /// same 32 bytes: the magic, the version, and the reserved bytes are fixed,
    /// and parsing refuses an object where any of them is not what it must be.
    pub(super) fn header_bytes(&self) -> [u8; Header::LEN] {
        self.header.to_bytes()
    }

    /// The chunk size this object was written with (spec: FM-6).
    pub fn chunk_size(&self) -> ChunkSize {
        self.header.chunk_size
    }

    /// Whether this Container is one-file or a Pack (spec: PK-15).
    pub fn kind(&self) -> ContainerKind {
        self.kind
    }

    /// How many zero bytes the stream's padding tail is (spec: FM-4).
    pub fn pad_len(&self) -> u64 {
        self.pad_len
    }

    /// The entry table, in plaintext stream order (spec: FM-9).
    pub fn entries(&self) -> &[EntryMetadata] {
        &self.entries
    }

    /// The entry table, taken out of the outline.
    pub fn into_entries(self) -> Vec<EntryMetadata> {
        self.entries
    }

    /// What the entry table records about the Entry at one Entry Path, if it
    /// holds one.
    ///
    /// At most one: an Entry Path identifies at most one Entry of a Container,
    /// as it does of the whole Library (spec: EP-5).
    pub fn entry_at(&self, path: &EntryPath) -> Option<&EntryMetadata> {
        self.entries.iter().find(|entry| &entry.path == path)
    }

    /// How many chunk messages the object carries (spec: FM-5).
    pub fn chunk_count(&self) -> u64 {
        self.layout.chunk_count()
    }

    /// Where the chunk sequence starts in the object (spec: FM-2).
    pub fn body_start(&self) -> u64 {
        self.layout.body_start()
    }

    /// The plaintext stream's length, padding tail included (spec: FM-4).
    pub fn plaintext_len(&self) -> u64 {
        self.layout.padded_len()
    }

    /// How long the whole object is, as its own header and meta section say.
    ///
    /// A reader that fetched the object whole can hold what arrived against
    /// this; a reader working in ranges never needs the number at all.
    pub fn object_len(&self) -> u64 {
        self.layout.object_len()
    }

    /// Every chunk of the object, as one run.
    pub fn all_chunks(&self) -> ChunkRun {
        ChunkRun::new(self.layout, 0, self.layout.chunk_count())
    }

    /// The chunks covering a range of the plaintext stream.
    ///
    /// A chunk is the smallest thing that authenticates (spec: FM-5), so the
    /// range is rounded out to chunk boundaries and the caller skips whatever
    /// the first chunk holds in front of what it asked for. An Entry of length
    /// zero still names the chunk it stands at, because a run covers at least
    /// one chunk.
    pub fn chunks_covering(&self, plaintext: Range<u64>) -> Result<ChunkRun> {
        if plaintext.start > plaintext.end || plaintext.end > self.layout.padded_len() {
            return Err(Error::PlaintextRangeOutOfBounds {
                start: plaintext.start,
                end: plaintext.end,
                plaintext_len: self.layout.padded_len(),
            });
        }

        let chunk_size = u64::from(self.header.chunk_size.get());
        let final_index = self.layout.final_index();
        let first = (plaintext.start / chunk_size).min(final_index);
        let last = if plaintext.end > plaintext.start {
            ((plaintext.end - 1) / chunk_size).min(final_index)
        } else {
            first
        };
        Ok(ChunkRun::new(self.layout, first, last - first + 1))
    }
}
