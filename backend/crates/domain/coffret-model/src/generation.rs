use crate::error::{Error, Result};
use std::fmt;

/// How many times a control object of one kind has been rewritten.
///
/// The generation counts that kind's own updates across the Library's whole
/// life and never restarts at a Master Key rotation, so a kind's object names
/// are never reused across epochs. It is what makes the newest Journal record
/// or Index Snapshot recognizable by name before any index exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(u64);

impl Generation {
    /// The generation the first object of a kind is written as.
    pub const FIRST: Self = Self(0);

    /// Takes a generation number.
    pub const fn new(generation: u64) -> Self {
        Self(generation)
    }

    /// The generation number.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The generation the next write of this object kind takes.
    pub fn next(self) -> Result<Self> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(Error::GenerationOutOfRange)
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_up_from_the_first_generation() {
        assert_eq!(Generation::FIRST.get(), 0);
        assert_eq!(
            Generation::FIRST.next().expect("0 has a successor"),
            Generation::new(1)
        );
    }

    #[test]
    fn the_last_representable_generation_has_no_successor() {
        assert_eq!(
            Generation::new(u64::MAX).next(),
            Err(Error::GenerationOutOfRange)
        );
    }
}
