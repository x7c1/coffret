use crate::error::{Error, Result};
use crate::format_integer::MAX_FORMAT_INTEGER;
use std::fmt;

/// Where a control object sits in the Library's control history.
///
/// Journal records and activation Index Snapshots form one head chain, each
/// successor taking the head's generation plus 1; an ordinary Index Snapshot
/// takes the generation of the head it checkpoints; a Keyring counts its own
/// envelope sets. None of them restarts at a Master Key rotation, so an object
/// name is never reused across epochs, and the newest Journal record or Index
/// Snapshot is recognizable by name before any index exists.
///
/// A generation is one of the integers the format bounds: it is at most
/// [`MAX_FORMAT_INTEGER`], so a number past that names no generation, and a
/// head chain counting commits never reaches one (spec: FM-19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(u64);

impl Generation {
    /// The generation the Library's first head, and its first Keyring, is
    /// written as.
    pub const FIRST: Self = Self(0);

    /// Takes a generation number, or refuses one the format does not admit.
    ///
    /// # Errors
    ///
    /// [`Error::GenerationOutOfRange`] where `generation` is past
    /// [`MAX_FORMAT_INTEGER`] (spec: FM-19).
    pub fn new(generation: u64) -> Result<Self> {
        if generation > MAX_FORMAT_INTEGER {
            return Err(Error::GenerationOutOfRange { generation });
        }
        Ok(Self(generation))
    }

    /// The generation number.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The generation the successor of this head, or the next Keyring set,
    /// takes.
    ///
    /// # Errors
    ///
    /// [`Error::GenerationOutOfRange`] where this is the last generation the
    /// format admits, which therefore has no successor to write next.
    pub fn next(self) -> Result<Self> {
        Self::new(self.0 + 1)
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

    /// The generation `number` stands for, or a panic naming the literal that
    /// stands for none.
    fn generation(number: u64) -> Generation {
        Generation::new(number)
            .unwrap_or_else(|error| panic!("a case holds a literal generation: {error}"))
    }

    #[test]
    fn counts_up_from_the_first_generation() {
        assert_eq!(Generation::FIRST.get(), 0);
        assert_eq!(
            Generation::FIRST.next().expect("0 has a successor"),
            generation(1)
        );
    }

    // FM-19: every integer the format carries is below 2^63, so a generation
    // at or past it names no control object — while the one just below it is
    // an ordinary generation that simply has nowhere left to count to.
    #[test]
    fn a_generation_past_the_formats_integer_range_is_refused() {
        let result = Generation::new(MAX_FORMAT_INTEGER + 1);
        assert!(
            matches!(
                result,
                Err(Error::GenerationOutOfRange { generation })
                    if generation == MAX_FORMAT_INTEGER + 1
            ),
            "expected 2^63 to name no generation, got {result:?}"
        );

        let last = generation(MAX_FORMAT_INTEGER);
        let successor = last.next();
        assert!(
            matches!(successor, Err(Error::GenerationOutOfRange { .. })),
            "expected the last generation to have no successor, got {successor:?}"
        );
    }
}
