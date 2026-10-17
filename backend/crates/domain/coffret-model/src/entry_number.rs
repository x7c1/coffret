use std::fmt;

use crate::error::{Error, Result};

/// An Entry's 0-based position in its Container's entry table (spec: FM-9).
///
/// A Container is immutable, so the order of its entry table never changes
/// once written, and a position in it is a name for one Entry that no rename
/// moves: `(Container ID, entry number)` names exactly one Entry, whatever the
/// catalog currently calls it. That is what lets a Journal record say which
/// Entry it renames without spelling the name it is taking away
/// (spec: FM-15), and what an Index Snapshot carries beside each Entry,
/// since its own order is the Entry Path order and loses the position
/// (spec: FM-16).
///
/// Numbers are assigned gap-free in the order the table was written: a Pack's
/// in its freeze's Entry Path order, a one-file Container's always 0. The
/// format carries one as an unsigned integer below 2^32.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryNumber(u32);

impl EntryNumber {
    /// The number of a one-file Container's only Entry, and of a Pack's first.
    pub const FIRST: Self = Self(0);

    /// The entry number `number` spells, or a refusal where the format admits
    /// none that large.
    ///
    /// # Errors
    ///
    /// [`Error::EntryNumberOutOfRange`] where `number` does not fit in 32 bits
    /// (spec: FM-9).
    pub fn new(number: u64) -> Result<Self> {
        u32::try_from(number)
            .map(Self)
            .map_err(|_| Error::EntryNumberOutOfRange { number })
    }

    /// The number of the Entry at `position` in an entry table, for a reader
    /// numbering a table it walks in order.
    ///
    /// # Errors
    ///
    /// [`Error::EntryNumberOutOfRange`] where the table is longer than entry
    /// numbers reach (spec: FM-9).
    pub fn at_position(position: usize) -> Result<Self> {
        Self::new(position as u64)
    }

    /// The number, as the format carries it.
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for EntryNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // FM-9: an entry number is a u32, so the first number past it names no
    // Entry, and a table position is numbered by where it stands.
    #[test]
    fn an_entry_number_is_a_32_bit_position() {
        assert_eq!(
            EntryNumber::new(0).expect("0 is a number"),
            EntryNumber::FIRST
        );
        assert_eq!(
            EntryNumber::new(u64::from(u32::MAX))
                .expect("the largest u32 is one")
                .get(),
            u32::MAX,
        );
        assert!(matches!(
            EntryNumber::new(u64::from(u32::MAX) + 1),
            Err(Error::EntryNumberOutOfRange { number }) if number == u64::from(u32::MAX) + 1
        ));
        assert_eq!(EntryNumber::at_position(3).expect("3 fits").get(), 3);
    }
}
