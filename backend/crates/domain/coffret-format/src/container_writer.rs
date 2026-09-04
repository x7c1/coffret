use coffret_model::ContentHash;

use crate::aead::Cipher;
use crate::encode_plan::EncodePlan;
use crate::error::{Error, Result};
use crate::header::Header;
use crate::layout::Layout;
use crate::nonce;

/// How many zero bytes the padding tail is appended in at a time.
///
/// The tail is bounded at about 12% of the stream (spec: FM-4), which for a
/// Pack is measured in megabytes, so it is fed through a slab rather than
/// materialized.
const ZERO_SLAB: usize = 64 * 1024;

#[cfg(test)]
mod tests;

/// Writes one Container without ever holding the whole of it.
///
/// [`encode`](crate::encode()) is handed every Entry's content and gives back the
/// finished object. That is the right shape for a Container the size of one
/// photo and the wrong one for a Pack: a normal Pack is around a gigabyte and an
/// oversized singleton is whatever one indivisible Entry happens to be
/// (spec: PK-3, PK-5). This writes the same bytes with a bounded appetite —
/// one chunk of plaintext and one chunk of ciphertext at a time — which is what
/// lets a Pack be spooled straight to disk.
///
/// It can do that because nothing in the entry table depends on reading the
/// content: [`EntryPlan`](crate::EntryPlan) declares each Entry's size and hash,
/// so the header and the meta section are settled before the first byte arrives
/// and the content only has to be pushed past the chunk boundary afterwards.
/// What the plan declares is then held to: the bytes fed for each Entry are
/// counted and hashed as they pass, and [`finish`](Self::finish) catches an
/// Entry the stream never delivered.
///
/// # On error, discard the writer and the bytes
///
/// Every step appends to `out` as it goes — the header and the sealed meta
/// section, then sealed chunks — and undoes nothing. On any error from
/// [`begin`](Self::begin), [`write`](Self::write), or [`finish`](Self::finish),
/// discard every byte appended to `out`, and the writer with it where one is
/// still held.
///
/// What the writer does guarantee is that such a Container is never *completed*.
/// The final chunk carries the final-chunk nonce domain (spec: FM-7) and only a
/// successful `finish` produces it, so a mismatched or short stream cannot be
/// decoded as a Container — it can only be an unfinished prefix on the way to
/// nowhere.
///
/// # Using it
///
/// Every step appends to a `Vec` the caller owns, so the caller decides where
/// the ciphertext goes and when the buffer is drained:
///
/// ```
/// use coffret_format::{ContainerWriter, EncodePlan, EntryPlan};
/// use coffret_model::{ContainerKey, ContainerKind, ContentHash, EntryPath, Mtime};
///
/// # fn main() -> coffret_format::Result<()> {
/// let key = ContainerKey::from_bytes([0x42; ContainerKey::BYTE_LEN]);
/// let content = b"the file's bytes";
/// let entries = [EntryPlan::new(
///     EntryPath::parse("photos/spring.jpg")?,
///     Mtime::from_unix_seconds(1_700_000_000),
///     content.len() as u64,
///     ContentHash::from_bytes(*blake3::hash(content).as_bytes()),
/// )];
///
/// let plan = EncodePlan::new(
///     coffret_format::generate_container_id()?,
///     ContainerKind::Pack,
///     &key,
///     &entries,
/// );
///
/// let mut object = Vec::new();
/// let mut writer = ContainerWriter::begin(&plan, &mut object)?;
/// writer.write(content, &mut object)?;
/// writer.finish(&mut object)?;
///
/// let opened = coffret_format::decode(&object, &key)?;
/// assert_eq!(opened.entries[0].content, content);
/// # Ok(())
/// # }
/// ```
///
/// A real caller writes `object` out and clears it between calls, which is what
/// keeps the memory flat however large the Container is.
pub struct ContainerWriter {
    cipher: Cipher,
    header_bytes: [u8; Header::LEN],
    /// What each Entry declared, in entry-table order.
    sizes: Vec<u64>,
    hashes: Vec<ContentHash>,
    /// Which Entry the next byte belongs to, and how much of it has arrived.
    entry_index: usize,
    entry_written: u64,
    hasher: blake3::Hasher,
    /// The plaintext of the chunk being filled.
    buffer: Vec<u8>,
    chunk_size: usize,
    chunk_count: u64,
    emitted: u64,
    pad_len: u64,
    /// How many content bytes the whole entry table plans for.
    planned: u64,
}

impl ContainerWriter {
    /// Starts a Container, appending the bytes it opens with to `out`.
    ///
    /// Those bytes are the header and the encrypted meta section — everything
    /// that is settled by the plan alone. What follows them is the chunk
    /// sequence, which [`write`](Self::write) and [`finish`](Self::finish)
    /// produce.
    pub fn begin(plan: &EncodePlan<'_>, out: &mut Vec<u8>) -> Result<Self> {
        let entries = plan
            .entries
            .iter()
            .map(|entry| entry.to_metadata(0))
            .collect();
        let mut layout = Layout::plan(plan.container_id, plan.chunk_size, plan.kind, entries)?;

        let cipher = Cipher::new(plan.key.as_bytes());
        out.extend_from_slice(&layout.header_bytes);
        cipher.seal(
            &nonce::meta(),
            &layout.header_bytes,
            &mut layout.meta_plaintext,
            out,
        )?;

        let chunk_size = usize::try_from(plan.chunk_size.get()).map_err(|_| {
            // A chunk size beyond this platform's addressable range is not one
            // this writer can buffer, even though the header could record it.
            Error::InvalidChunkSize
        })?;
        let planned = plan.entries.iter().try_fold(0u64, |total, entry| {
            total.checked_add(entry.size).ok_or(Error::StreamTooLong)
        })?;

        Ok(Self {
            cipher,
            header_bytes: layout.header_bytes,
            sizes: plan.entries.iter().map(|entry| entry.size).collect(),
            hashes: plan.entries.iter().map(|entry| entry.hash).collect(),
            entry_index: 0,
            entry_written: 0,
            hasher: blake3::Hasher::new(),
            // A stream shorter than one chunk needs no more buffer than it
            // fills.
            buffer: Vec::with_capacity(
                usize::try_from(layout.padded_len.min(chunk_size as u64).max(1))
                    .expect("the buffer is at most one chunk long"),
            ),
            chunk_size,
            chunk_count: layout.chunk_count,
            emitted: 0,
            pad_len: layout.pad_len,
            planned,
        })
    }

    /// Feeds the next bytes of the plaintext stream, appending whatever chunks
    /// they complete to `out`.
    ///
    /// The stream is every Entry's content back to back in entry-table order, so
    /// a caller hands over one file after another and never has to say where one
    /// Entry ends: the plan already did.
    pub fn write(&mut self, plaintext: &[u8], out: &mut Vec<u8>) -> Result<()> {
        let mut remaining = plaintext;
        while !remaining.is_empty() {
            self.close_filled_entries()?;
            let Some(size) = self.sizes.get(self.entry_index) else {
                return Err(Error::StreamOverrun {
                    planned: self.planned,
                });
            };
            let left = usize::try_from(size - self.entry_written).unwrap_or(usize::MAX);
            let take = left.min(remaining.len());
            self.hasher.update(&remaining[..take]);
            self.entry_written += take as u64;
            self.push(&remaining[..take], out)?;
            remaining = &remaining[take..];
        }
        Ok(())
    }

    /// Closes the Container, appending the padding tail and the final chunk to
    /// `out`.
    ///
    /// This is where an Entry the stream never delivered is caught, and where
    /// the final-chunk nonce domain is produced (spec: FM-7). Only a successful
    /// call completes a Container, so an object whose entry table describes
    /// bytes that are not in it can never be decoded as one.
    ///
    /// On error, discard every byte already appended to `out`: nothing appended
    /// by this call or an earlier one is rolled back.
    pub fn finish(mut self, out: &mut Vec<u8>) -> Result<()> {
        self.close_filled_entries()?;
        if let Some(size) = self.sizes.get(self.entry_index) {
            return Err(Error::EntryLengthMismatch {
                index: self.entry_index,
                expected: *size,
                actual: self.entry_written,
            });
        }

        let zeros = [0u8; ZERO_SLAB];
        let mut padding_left = self.pad_len;
        while padding_left > 0 {
            let take = usize::try_from(padding_left)
                .unwrap_or(usize::MAX)
                .min(ZERO_SLAB);
            self.push(&zeros[..take], out)?;
            padding_left -= take as u64;
        }

        // Whatever is still in the buffer is the last chunk, however short —
        // including the empty one an all-empty entry table leaves (spec: FM-5).
        self.seal(true, out)?;
        debug_assert_eq!(
            self.emitted, self.chunk_count,
            "the layout's chunk count is what the writer emits",
        );
        Ok(())
    }

    /// Settles every Entry whose declared bytes have all arrived.
    ///
    /// A run of them can close at once, because an Entry of length zero is full
    /// the moment it starts.
    fn close_filled_entries(&mut self) -> Result<()> {
        while let Some(size) = self.sizes.get(self.entry_index) {
            if self.entry_written != *size {
                return Ok(());
            }
            let hash = ContentHash::from_bytes(*self.hasher.finalize().as_bytes());
            if hash != self.hashes[self.entry_index] {
                return Err(Error::EntryHashMismatch {
                    index: self.entry_index,
                });
            }
            self.hasher.reset();
            self.entry_index += 1;
            self.entry_written = 0;
        }
        Ok(())
    }

    /// Puts stream bytes into the chunk buffer, sealing it each time it fills.
    ///
    /// The last chunk is never sealed here even when it fills exactly: it is
    /// [`finish`](Self::finish)'s, because its nonce carries the final-chunk
    /// domain that marks the end of the stream (spec: FM-7).
    fn push(&mut self, mut bytes: &[u8], out: &mut Vec<u8>) -> Result<()> {
        while !bytes.is_empty() {
            let room = self.chunk_size - self.buffer.len();
            let take = room.min(bytes.len());
            self.buffer.extend_from_slice(&bytes[..take]);
            bytes = &bytes[take..];
            if self.buffer.len() == self.chunk_size && self.emitted + 1 < self.chunk_count {
                self.seal(false, out)?;
            }
        }
        Ok(())
    }

    /// Encrypts the buffered chunk into `out` and empties the buffer.
    fn seal(&mut self, is_final: bool, out: &mut Vec<u8>) -> Result<()> {
        self.cipher.seal(
            &nonce::chunk(self.emitted, is_final),
            &self.header_bytes,
            &mut self.buffer,
            out,
        )?;
        self.buffer.clear();
        self.emitted += 1;
        Ok(())
    }
}
