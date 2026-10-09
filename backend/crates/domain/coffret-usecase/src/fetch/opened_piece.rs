use std::ops::Range;

use coffret_format::Error as FormatError;

use crate::fetch::fetch_error::{FetchError, FetchResult};

/// One piece of a Container's plaintext stream as the chunk decoder handed it
/// over, and where that piece stands in the stream.
///
/// It exists so that the arithmetic between a stream position and an index into
/// a buffer is done once. A position is a `u64` because a Container's stream is
/// addressed in 64 bits (spec: FM-4, FM-9) and an index is a `usize` because a
/// buffer is as long as this machine can address, and a cast between the two
/// made in passing is where a 32-bit reader would quietly deliver the wrong
/// bytes. Every conversion here is of a distance from this piece's own start to
/// a position inside it, which is at most the piece's own length — so the
/// conversion cannot fail, and it says so rather than truncating.
pub(super) struct OpenedPiece<'a> {
    /// Where this piece's first byte stands in the plaintext stream.
    start: u64,
    /// The bytes themselves.
    bytes: &'a [u8],
}

impl<'a> OpenedPiece<'a> {
    /// The piece standing at `start`, or the refusal a stream reaching past
    /// what 64 bits can address earns.
    ///
    /// No writer produces such a Container — the layout one is written from
    /// refuses an entry table that would need it (spec: FM-9) — so this is the
    /// walk saying so instead of wrapping round and writing bytes from the
    /// wrong place into a file.
    pub(super) fn at(start: u64, bytes: &'a [u8]) -> FetchResult<Self> {
        match start.checked_add(bytes.len() as u64) {
            Some(_) => Ok(Self { start, bytes }),
            None => Err(FetchError::Format(FormatError::StreamTooLong)),
        }
    }

    /// The first stream position past this piece.
    pub(super) fn end(&self) -> u64 {
        self.start + self.bytes.len() as u64
    }

    /// The bytes of this piece that belong to `wanted`, where any of them do.
    pub(super) fn overlapping(&self, wanted: &Range<u64>) -> Option<&'a [u8]> {
        let from = wanted.start.max(self.start);
        let to = wanted.end.min(self.end());
        (from < to).then(|| &self.bytes[self.index_of(from)..self.index_of(to)])
    }

    /// The part of this piece from `position` on, where the piece reaches it.
    ///
    /// What a read that is asked again whole keeps of its second answer: the
    /// bytes it had already handed on before the first answer stopped are the
    /// same bytes, every chunk having authenticated as exactly that chunk
    /// (spec: FM-7), so they are stepped over rather than written twice.
    pub(super) fn from(&self, position: u64) -> Option<OpenedPiece<'a>> {
        if position >= self.end() {
            return None;
        }
        let start = position.max(self.start);
        Some(OpenedPiece {
            start,
            bytes: &self.bytes[self.index_of(start)..],
        })
    }

    /// Where this piece's first byte stands in the plaintext stream.
    pub(super) fn start(&self) -> u64 {
        self.start
    }

    /// How far into this piece a stream position inside it stands.
    fn index_of(&self, position: u64) -> usize {
        usize::try_from(position - self.start)
            .expect("a position inside one opened piece is no further in than its own length")
    }
}
