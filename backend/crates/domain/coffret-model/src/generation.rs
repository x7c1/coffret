use crate::error::{Error, Result};
use std::fmt;

/// Where a control object sits in the Library's control history.
///
/// Journal records and activation Index Snapshots form one head chain, each
/// successor taking the head's generation plus 1; an ordinary Index Snapshot
/// takes the generation of the head it checkpoints; a Keyring counts its own
/// envelope sets. None of them restarts at a Master Key rotation, so an object
/// name is never reused across epochs, and the newest Journal record or Index
/// Snapshot is recognizable by name before any index exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(u64);

impl Generation {
    /// The generation the Library's first head, and its first Keyring, is
    /// written as.
    pub const FIRST: Self = Self(0);

    /// Takes a generation number.
    pub const fn new(generation: u64) -> Self {
        Self(generation)
    }

    /// The generation number.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The generation the successor of this head, or the next Keyring set,
    /// takes.
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
        let result = Generation::new(u64::MAX).next();
        assert!(
            matches!(result, Err(Error::GenerationOutOfRange)),
            "expected the last generation to have no successor, got {result:?}"
        );
    }
}
