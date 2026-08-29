use crate::aead::TAG_LEN;
use crate::error::{Error, Result};
use crate::header::Header;

/// Where a Container's chunks sit, in the object and in the plaintext stream.
///
/// Every value here follows from the header and the meta section alone
/// (spec: FM-2, FM-4, FM-5, FM-6), which is the whole reason a reader can aim a
/// range read at one Entry before it has seen a byte of the chunk sequence.
///
/// The arithmetic is done once, here, and checked once: [`ChunkLayout::of`]
/// refuses a layout whose object would not fit in a `u64`, so every method
/// below is plain arithmetic on values already known to be in range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChunkLayout {
    /// Plaintext bytes per chunk, as the header records it (spec: FM-6).
    chunk_size: u64,
    /// How many chunk messages the object carries (spec: FM-5).
    chunk_count: u64,
    /// Where the chunk sequence starts: the header and the meta section are in
    /// front of it (spec: FM-2).
    body_start: u64,
    /// The plaintext stream's length once the padding tail is on it
    /// (spec: FM-4).
    padded_len: u64,
    /// How long the whole object is.
    object_len: u64,
}

impl ChunkLayout {
    /// Works out one Container's chunk layout from its header and the stream
    /// length its meta section describes.
    pub(crate) fn of(header: &Header, padded_len: u64) -> Result<Self> {
        let chunk_size = u64::from(header.chunk_size.get());
        // Entries that are all empty still produce one empty final chunk, so
        // the sequence is never empty (spec: FM-5).
        let chunk_count = padded_len.div_ceil(chunk_size).max(1);
        let body_start = Header::LEN as u64 + u64::from(header.meta_len);
        let object_len = chunk_count
            .checked_mul(TAG_LEN as u64)
            .and_then(|tags| tags.checked_add(padded_len))
            .and_then(|body| body.checked_add(body_start))
            .ok_or(Error::StreamTooLong)?;

        Ok(Self {
            chunk_size,
            chunk_count,
            body_start,
            padded_len,
            object_len,
        })
    }

    /// How many chunk messages the object carries.
    pub(crate) fn chunk_count(&self) -> u64 {
        self.chunk_count
    }

    /// Index of the object's final chunk, the one FM-7's final domain marks.
    pub(crate) fn final_index(&self) -> u64 {
        self.chunk_count - 1
    }

    /// Where the chunk sequence starts in the object.
    pub(crate) fn body_start(&self) -> u64 {
        self.body_start
    }

    /// The padded plaintext stream's length (spec: FM-4).
    pub(crate) fn padded_len(&self) -> u64 {
        self.padded_len
    }

    /// How long the whole object is.
    pub(crate) fn object_len(&self) -> u64 {
        self.object_len
    }

    /// Where the chunk at `index` starts in the plaintext stream.
    pub(crate) fn plaintext_start_of(&self, index: u64) -> u64 {
        index * self.chunk_size
    }

    /// How many plaintext bytes the chunk at `index` carries.
    ///
    /// Every chunk but the last is exactly one chunk size; the last keeps the
    /// remainder, which is empty for a stream that is empty (spec: FM-5).
    pub(crate) fn plaintext_len_of(&self, index: u64) -> u64 {
        if index < self.final_index() {
            self.chunk_size
        } else {
            self.padded_len - self.final_index() * self.chunk_size
        }
    }

    /// How long the chunk at `index`'s AEAD message is, its tag included.
    pub(crate) fn message_len_of(&self, index: u64) -> u64 {
        self.plaintext_len_of(index) + TAG_LEN as u64
    }

    /// Where the chunk at `index`'s message starts in the object.
    pub(crate) fn message_start_of(&self, index: u64) -> u64 {
        self.body_start + index * (self.chunk_size + TAG_LEN as u64)
    }
}
