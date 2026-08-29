use std::ops::Range;

use crate::container_reader::chunk_layout::ChunkLayout;

/// A run of consecutive chunks of one Container, and where its bytes lie.
///
/// A run is what a reader asks Storage for and what it then opens: the byte
/// range [`ciphertext`](Self::ciphertext) names is exactly the messages of the
/// chunks in it, and [`plaintext_start`](Self::plaintext_start) says where in
/// the Container's plaintext stream the bytes that come back begin. An Entry is
/// reached by rounding its own extent out to the chunks that cover it, because
/// a chunk is the smallest thing that authenticates (spec: FM-5, PK-16).
///
/// It carries the layout it was cut from, so a caller never has to hold the
/// outline and the run together to make sense of either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkRun {
    layout: ChunkLayout,
    first: u64,
    count: u64,
}

impl ChunkRun {
    /// A run of `count` chunks starting at `first`.
    ///
    /// Private because a run that reaches past the object is not a thing a
    /// caller can act on: they come from
    /// [`ContainerOutline`](super::ContainerOutline), which cuts them from a
    /// layout it read out of the object itself.
    pub(super) fn new(layout: ChunkLayout, first: u64, count: u64) -> Self {
        debug_assert!(count >= 1, "a chunk run covers at least one chunk");
        debug_assert!(
            first + count <= layout.chunk_count(),
            "a chunk run stays inside the object it was cut from",
        );
        Self {
            layout,
            first,
            count,
        }
    }

    /// Index of the run's first chunk, counted from 0 across the object
    /// (spec: FM-7).
    pub fn first(&self) -> u64 {
        self.first
    }

    /// How many chunks the run covers, always at least one.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Where the run's first plaintext byte stands in the Container's plaintext
    /// stream.
    ///
    /// The run starts at a chunk boundary, so this is at or before the Entry a
    /// caller aimed it at: the bytes in front of the Entry are the tail of
    /// whatever shares its first chunk, and skipping them is the caller's.
    pub fn plaintext_start(&self) -> u64 {
        self.layout.plaintext_start_of(self.first)
    }

    /// The object byte range the run's ciphertext occupies.
    ///
    /// This is what a caller hands a Storage range read, and it is exact rather
    /// than generous: every chunk but the last is one chunk size plus a tag,
    /// and the last is whatever remains (spec: FM-5).
    pub fn ciphertext(&self) -> Range<u64> {
        let last = self.first + self.count - 1;
        self.layout.message_start_of(self.first)
            ..self.layout.message_start_of(last) + self.layout.message_len_of(last)
    }

    /// The layout the run was cut from, for the reader that opens it.
    pub(super) fn layout(&self) -> ChunkLayout {
        self.layout
    }
}
