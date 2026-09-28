use std::num::NonZeroUsize;

/// The replica positions one repair rewrote: at least one, in ascending order.
///
/// Its own type so that "a repair rewrote something" is held where the value is
/// made rather than by every reader: the first position is a field of its own,
/// so there is no spelling of one that names none, and a reader counting the
/// positions gets a count that cannot be zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewrittenReplicas {
    first: u16,
    rest: Vec<u16>,
}

impl RewrittenReplicas {
    /// The positions a walk put back, or `None` where it put none back.
    pub(crate) fn new(positions: Vec<u16>) -> Option<Self> {
        let mut positions = positions.into_iter();
        let first = positions.next()?;
        Some(Self {
            first,
            rest: positions.collect(),
        })
    }

    /// The positions a repair put back, put together by hand rather than by a
    /// walk.
    ///
    /// For a shell's own cases alone, behind a feature only a shell's
    /// `[dev-dependencies]` turn on: a shell has to say what it prints for a
    /// repair, and a repair is otherwise only ever made by a commit's
    /// examination, so no shipping caller can report one no run performed.
    #[cfg(feature = "assembled-repairs")]
    pub fn assembled(positions: Vec<u16>) -> Option<Self> {
        Self::new(positions)
    }

    /// The positions, in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = u16> + '_ {
        std::iter::once(self.first).chain(self.rest.iter().copied())
    }

    /// How many positions were rewritten, which is at least one.
    #[must_use]
    pub fn count(&self) -> NonZeroUsize {
        NonZeroUsize::MIN.saturating_add(self.rest.len())
    }
}
