//! The parts of a Container that are settled before a byte of content moves.
//!
//! The header, the meta section, and the shape of the chunk sequence all follow
//! from the entry table alone. [`encode`](crate::encode) has the content in hand
//! and [`ContainerWriter`](crate::ContainerWriter) has not yet seen any of it,
//! and they still have to lay out the same object — so the layout is worked out
//! here once and each of them only walks the plaintext stream its own way.

use coffret_model::{ContainerId, ContainerKind, EntryMetadata};

use crate::aead::TAG_LEN;
use crate::chunk_size::ChunkSize;
use crate::error::{Error, Result};
use crate::header::Header;
use crate::meta::{self, Meta};
use crate::padme;

/// A Container's fixed parts, ready to be written.
pub(crate) struct Layout {
    /// The 32 plaintext bytes the object starts with, and the associated data of
    /// every AEAD message in it.
    pub(crate) header_bytes: [u8; Header::LEN],
    /// The meta section's plaintext, carried to its Padmé bucket (spec: FM-9).
    pub(crate) meta_plaintext: Vec<u8>,
    /// How long the encrypted meta section is, tag included.
    pub(crate) meta_len: u32,
    /// The plaintext stream's length once the padding tail is on it.
    pub(crate) padded_len: u64,
    /// How many zero bytes that tail is.
    pub(crate) pad_len: u64,
    /// How many chunk messages the object carries (spec: FM-5).
    pub(crate) chunk_count: u64,
}

impl Layout {
    /// Works out one Container's fixed parts from its entry table.
    ///
    /// The offsets are assigned here rather than taken from the caller, for the
    /// reason the encoder derives them from the content it is given: an offset
    /// that disagrees with the stream is a Container nothing can read back, and
    /// two callers stating the same rule are two chances to state it
    /// differently.
    pub(crate) fn plan(
        container_id: ContainerId,
        chunk_size: ChunkSize,
        kind: ContainerKind,
        mut entries: Vec<EntryMetadata>,
    ) -> Result<Self> {
        // A Container exists only to hold user data, so an empty one is not a
        // Container worth writing.
        if entries.is_empty() {
            return Err(Error::EmptyEntryTable);
        }

        let mut offset = 0u64;
        for entry in &mut entries {
            entry.offset = offset;
            offset = offset.checked_add(entry.size).ok_or(Error::StreamTooLong)?;
        }

        let unpadded_len = offset;
        let padded_len = padme::padded_len(unpadded_len);
        let meta = Meta {
            kind,
            pad_len: padded_len - unpadded_len,
            entries,
        };

        // The header's associated data covers the meta section length, so the
        // meta section has to be serialized before the header can be written.
        //
        // Its plaintext is the CBOR map followed by zero padding up to the next
        // Padmé bucket, so the length the header records blurs the Entry count
        // and the total Entry Path length the way the stream padding blurs the
        // content. CBOR is self-delimiting, so the decoder needs no length field
        // to tell the map from the padding.
        let mut meta_plaintext = meta::encode(&meta)?;
        // The header records the padded section with its tag in one 32-bit
        // field, and the section is materialized in memory, so a meta section
        // fits under whichever of those two ceilings is lower.
        let limit = (u64::from(u32::MAX) - TAG_LEN as u64).min(usize::MAX as u64);
        let padded = padme::padded_len(meta_plaintext.len() as u64);
        if padded > limit {
            return Err(Error::MetaSectionTooLong { padded, limit });
        }
        let padded_meta_len = usize::try_from(padded).expect("checked against the ceiling above");
        meta_plaintext.resize(padded_meta_len, 0);
        let meta_len =
            u32::try_from(padded_meta_len + TAG_LEN).expect("checked against the ceiling");

        let header_bytes = Header {
            container_id,
            chunk_size,
            meta_len,
        }
        .to_bytes();

        // Entries that are all empty still produce one empty final chunk, so
        // every object ends with a final-chunk message marking the end of the
        // stream.
        let chunk_count = padded_len.div_ceil(u64::from(chunk_size.get())).max(1);

        Ok(Self {
            header_bytes,
            meta_plaintext,
            meta_len,
            padded_len,
            pad_len: meta.pad_len,
            chunk_count,
        })
    }

    /// How many bytes the finished object will be, for a caller sizing a buffer.
    pub(crate) fn object_len_hint(&self) -> usize {
        let chunk_bytes = self
            .padded_len
            .saturating_add(self.chunk_count.saturating_mul(TAG_LEN as u64));
        Header::LEN + self.meta_len as usize + usize::try_from(chunk_bytes).unwrap_or(0)
    }
}
