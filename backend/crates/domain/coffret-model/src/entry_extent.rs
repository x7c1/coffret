use std::ops::Range;

use crate::error::{Error, Result};
use crate::format_integer::MAX_FORMAT_INTEGER;

/// Where one Entry's plaintext lies in its Container's plaintext stream
/// (spec: FM-9).
///
/// The offset and the length are one value because neither answers anything on
/// its own: what a range read of a single Entry out of a Pack is aimed with is
/// the pair (spec: PK-16), and every reader that had them apart went on to add
/// them together. Carrying them together is what makes the one condition they
/// have an invariant instead of a check each caller remembers to make — the
/// extent ends inside the address space the format admits, so `offset + size`
/// is at most [`MAX_FORMAT_INTEGER`] (spec: FM-19).
///
/// That is the whole of what is refused here. Whether a table of these tiles
/// its stream from zero without gaps or overlaps is a rule about the table
/// rather than about any one Entry, and the decoders that read a table keep
/// checking it (spec: FM-9, FM-10).
///
/// A zero-length extent is an extent. An Entry of no bytes is a file of no
/// bytes, which a Library holds like any other, and a writer laying one down
/// gives it the position the walk had reached (spec: FM-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryExtent {
    offset: u64,
    size: u64,
}

impl EntryExtent {
    /// The extent `size` bytes long that starts at `offset`, or a refusal where
    /// it would end past what the plaintext stream can address.
    ///
    /// # Errors
    ///
    /// [`Error::ExtentPastTheAddressSpace`] where `offset + size` is past
    /// [`MAX_FORMAT_INTEGER`], the last position the format admits (FM-19) —
    /// which covers a sum that overflows `u64` outright.
    pub fn new(offset: u64, size: u64) -> Result<Self> {
        match offset.checked_add(size) {
            Some(end) if end <= MAX_FORMAT_INTEGER => Ok(Self { offset, size }),
            _ => Err(Error::ExtentPastTheAddressSpace { offset, size }),
        }
    }

    /// The extent `size` bytes long at the start of a stream: the first Entry
    /// of one, or — where `size` is zero — the empty extent a tiling walk
    /// begins from.
    ///
    /// # Errors
    ///
    /// [`Error::ExtentPastTheAddressSpace`], on [`new`](Self::new)'s terms: a
    /// stream starting at zero still ends inside the address space, so a length
    /// the format does not admit is refused here as anywhere else.
    pub fn from_start(size: u64) -> Result<Self> {
        Self::new(0, size)
    }

    /// The extent of the Entry laid directly after this one, or a refusal where
    /// the stream would then run past its address space.
    ///
    /// The tiling walk an entry table is assigned by, in the one place that
    /// assigns it: every Entry begins where its predecessor ended (spec: FM-9).
    ///
    /// # Errors
    ///
    /// [`Error::ExtentPastTheAddressSpace`], on [`new`](Self::new)'s terms.
    pub fn following(&self, size: u64) -> Result<Self> {
        Self::new(self.end(), size)
    }

    /// Where this Entry's plaintext starts in the stream.
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    /// How many bytes of the stream belong to it.
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// The first stream position after it.
    ///
    /// Exclusive, and it never overflows: that the sum stays inside the address
    /// space is what [`new`](Self::new) refuses on, so every value of this type
    /// has an end to answer with.
    pub const fn end(&self) -> u64 {
        self.offset + self.size
    }

    /// The stream positions this Entry occupies, which is what a reader rounds
    /// out to the chunks covering it (spec: FM-5, PK-16).
    pub fn range(&self) -> Range<u64> {
        self.offset..self.end()
    }

    /// Whether `position` is one of the stream positions this Entry occupies.
    ///
    /// Half-open, as the range is: the end is the first position past the
    /// Entry, so a zero-length extent contains nothing at all.
    pub const fn contains(&self, position: u64) -> bool {
        self.offset <= position && position < self.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The extent `offset`/`size` stands for, or a panic naming the literal
    /// that stands for none.
    fn extent(offset: u64, size: u64) -> EntryExtent {
        EntryExtent::new(offset, size)
            .unwrap_or_else(|error| panic!("a case holds a literal extent: {error}"))
    }

    // FM-9, FM-19: an Entry is placed against a plaintext stream whose
    // positions the format bounds, so an extent whose end is not a position in
    // that stream places nothing. Every way of running past the end is the same
    // refusal, and it carries both numbers because either of them alone says
    // nothing about which pair was refused.
    #[test]
    fn an_extent_past_the_end_of_the_address_space_cannot_exist() {
        for (offset, size) in [(u64::MAX, 1), (u64::MAX - 1, 2), (1, u64::MAX)] {
            let result = EntryExtent::new(offset, size);
            assert!(
                matches!(
                    result,
                    Err(Error::ExtentPastTheAddressSpace {
                        offset: refused_offset,
                        size: refused_size,
                    }) if refused_offset == offset && refused_size == size
                ),
                "expected {offset}/{size} to be refused with both values, got {result:?}"
            );
        }
    }

    // FM-19: the bound is on the extent's end, so an extent ending at 2^63 is
    // refused however it is spelled and one ending a byte below it is not.
    #[test]
    fn an_extent_ending_past_the_formats_integer_range_is_refused() {
        for (offset, size) in [
            (MAX_FORMAT_INTEGER, 1),
            (1, MAX_FORMAT_INTEGER),
            (0, 1 << 63),
        ] {
            let result = EntryExtent::new(offset, size);
            assert!(
                matches!(result, Err(Error::ExtentPastTheAddressSpace { .. })),
                "expected an extent ending at 2^63 to be refused, got {result:?}"
            );
        }

        assert_eq!(
            extent(1, MAX_FORMAT_INTEGER - 1).end(),
            MAX_FORMAT_INTEGER,
            "an extent may end at the last position the format admits",
        );
        assert!(
            matches!(
                EntryExtent::from_start(1 << 63),
                Err(Error::ExtentPastTheAddressSpace { .. })
            ),
            "a stream starting at zero is bounded the same way",
        );
    }

    // FM-4: a file of no bytes is a file, and the extent a writer lays it at is
    // the position the walk had reached. It occupies nothing, which is exactly
    // what the half-open range says.
    #[test]
    fn a_zero_length_extent_is_an_extent() {
        let empty = extent(48, 0);

        assert_eq!(empty.end(), 48, "it ends where it begins");
        assert!(empty.range().is_empty(), "and covers no stream position");
        assert!(
            !empty.contains(48),
            "including the one it begins at, which belongs to whatever follows",
        );
    }

    // PK-16: the three questions a range read asks of an Entry's place — where
    // it ends, which stream positions to round out to chunks, and whether a
    // position the reader is standing at is one of them.
    #[test]
    fn an_extent_answers_its_end_range_and_what_it_contains() {
        let entry = extent(100, 40);

        assert_eq!(entry.offset(), 100);
        assert_eq!(entry.size(), 40);
        assert_eq!(entry.end(), 140);
        assert_eq!(entry.range(), 100..140);

        assert!(!entry.contains(99), "the byte before it belongs to another");
        assert!(entry.contains(100), "its first byte is its own");
        assert!(entry.contains(139), "and so is its last");
        assert!(
            !entry.contains(140),
            "the end is the first position past it",
        );
    }

    // FM-9: the entry table tiles the stream, so the extent after one starts
    // where it ended — and a table that would run off the end is refused at the
    // Entry that runs off it.
    #[test]
    fn the_extent_after_one_begins_where_it_ended() {
        let first = extent(0, 12);
        let second = first.following(30).expect("a stream of 42 bytes fits");

        assert_eq!(second.range(), 12..42);
        assert!(
            matches!(
                extent(1, MAX_FORMAT_INTEGER - 1).following(1),
                Err(Error::ExtentPastTheAddressSpace { .. })
            ),
            "an Entry laid after the last addressable byte is refused",
        );
    }
}
