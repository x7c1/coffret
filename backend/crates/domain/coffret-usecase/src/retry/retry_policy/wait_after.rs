use std::time::Duration;

use super::full_jitter::full_jitter;
use super::RetryPolicy;
use crate::error::Error;

impl RetryPolicy {
    /// The interval the wait after `waits_made` earlier waits is drawn from.
    ///
    /// Doubling from the base and stopping at the ceiling, so the interval
    /// widens while a provider might still be recovering and stops widening
    /// where waiting longer would only be waiting longer.
    pub(in crate::retry) fn interval(&self, waits_made: u32) -> Duration {
        let doubling = 1u32.checked_shl(waits_made);
        let widened = doubling.and_then(|factor| self.base_backoff.checked_mul(factor));
        // Past what a `Duration` can hold is past the ceiling by any reading.
        widened.unwrap_or(self.wait_ceiling).min(self.wait_ceiling)
    }

    /// How long to wait before the next attempt.
    pub(super) fn wait_after(&self, waits_made: u32, asked: Option<Duration>) -> Duration {
        match asked {
            // The provider named a figure. It knows when it will serve again,
            // and drawing something shorter only spends an attempt finding that
            // out — but it does not get to decide how long a worker is gone, so
            // the figure is clamped.
            Some(asked) => asked.min(self.wait_ceiling),
            None => full_jitter(self.interval(waits_made)),
        }
    }
}

/// How long the provider asked to be left alone, where it said.
pub(super) fn retry_after(error: &Error) -> Option<Duration> {
    match error {
        Error::RateLimited { retry_after, .. } => *retry_after,
        _ => None,
    }
}
