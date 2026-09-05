use crate::error::{Error, Result};
use crate::format_integer::MAX_FORMAT_INTEGER;
use std::fmt;

/// Which Master Key encrypted a piece of control state.
///
/// The Library's first epoch is 1, and each Master Key rotation increments it
/// by 1. The epoch is distinct from a control object's generation, which places
/// the object in the Library's control history.
///
/// The numbering runs from 1 to [`MAX_FORMAT_INTEGER`]: an epoch is one of the
/// integers the format bounds, and counting rotations never reaches the top of
/// that range (spec: FM-19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MasterKeyEpoch(u64);

impl MasterKeyEpoch {
    /// The epoch a Library starts life in.
    pub const FIRST: Self = Self(1);

    /// Takes an epoch number, which starts at 1.
    ///
    /// # Errors
    ///
    /// [`Error::EpochOutOfRange`] where `epoch` is 0, which names no epoch, or
    /// past [`MAX_FORMAT_INTEGER`], which the format does not admit (FM-19).
    pub fn new(epoch: u64) -> Result<Self> {
        if epoch == 0 || epoch > MAX_FORMAT_INTEGER {
            return Err(Error::EpochOutOfRange { epoch });
        }
        Ok(Self(epoch))
    }

    /// The epoch number.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The epoch a rotation from this one activates.
    ///
    /// # Errors
    ///
    /// [`Error::EpochOutOfRange`] where this is the last epoch the format
    /// admits, which therefore has no successor to rotate into.
    pub fn next(self) -> Result<Self> {
        Self::new(self.0 + 1)
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
            matches!(result, Err(Error::EpochOutOfRange { epoch: 0 })),
            "expected 0 to name no epoch, got {result:?}"
        );
    }

    // FM-19: every integer the format carries is below 2^63, so an epoch at or
    // past it names no Master Key — while the one just below it is an ordinary
    // epoch with nowhere left to rotate into.
    #[test]
    fn an_epoch_past_the_formats_integer_range_is_refused() {
        let result = MasterKeyEpoch::new(MAX_FORMAT_INTEGER + 1);
        assert!(
            matches!(
                result,
                Err(Error::EpochOutOfRange { epoch }) if epoch == MAX_FORMAT_INTEGER + 1
            ),
            "expected 2^63 to name no epoch, got {result:?}"
        );

        let last = MasterKeyEpoch::new(MAX_FORMAT_INTEGER).expect("the bound is an epoch");
        let successor = last.next();
        assert!(
            matches!(successor, Err(Error::EpochOutOfRange { .. })),
            "expected the last epoch to have no successor, got {successor:?}"
        );
    }
}
