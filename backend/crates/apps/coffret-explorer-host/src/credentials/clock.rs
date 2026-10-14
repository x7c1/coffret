use std::time::Instant;

/// Where [`Credentials`](super::Credentials) reads the time from.
///
/// A trait rather than [`Instant::now`] called in place, so that a case can
/// reach a ticket's expiry, a session's, or the exchange limit's recovery by
/// moving a clock of its own rather than by sleeping for a minute or twelve
/// hours. Monotonic, like `Instant`: a wall clock set back would otherwise
/// make an expired token good again.
pub trait Clock: Send + Sync {
    /// The time now.
    fn now(&self) -> Instant;
}
