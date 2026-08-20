use crate::error::{Error, Result};
use std::fmt;

/// Which Master Key encrypted a piece of control state.
///
/// The Library's first epoch is 1, and each Master Key rotation increments it
/// by 1. The epoch is distinct from a control object's generation, which counts
/// that object kind's own updates across the Library's whole life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MasterKeyEpoch(u64);

impl MasterKeyEpoch {
    /// The epoch a Library starts life in.
    pub const FIRST: Self = Self(1);

    /// Takes an epoch number, which starts at 1.
    pub fn new(epoch: u64) -> Result<Self> {
        if epoch == 0 {
            return Err(Error::EpochOutOfRange);
        }
        Ok(Self(epoch))
    }

    /// The epoch number.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The epoch a rotation from this one activates.
    pub fn next(self) -> Result<Self> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(Error::EpochOutOfRange)
    }
}

impl fmt::Display for MasterKeyEpoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // FM-13: the epoch is 1 for the Library's first epoch, incremented by 1 at
    // each rotation.
    #[test]
    fn numbering_starts_at_one_and_increments() {
        assert_eq!(MasterKeyEpoch::FIRST.get(), 1);
        assert_eq!(
            MasterKeyEpoch::FIRST.next().expect("1 has a successor"),
            MasterKeyEpoch::new(2).expect("2 is a valid epoch")
        );
    }

    #[test]
    fn zero_is_not_an_epoch() {
        let result = MasterKeyEpoch::new(0);
        assert!(
            matches!(result, Err(Error::EpochOutOfRange)),
            "expected 0 to name no epoch, got {result:?}"
        );
    }

    #[test]
    fn the_last_representable_epoch_has_no_successor() {
        let last = MasterKeyEpoch::new(u64::MAX).expect("u64::MAX is a valid epoch");
        let result = last.next();
        assert!(
            matches!(result, Err(Error::EpochOutOfRange)),
            "expected the last epoch to have no successor, got {result:?}"
        );
    }
}
