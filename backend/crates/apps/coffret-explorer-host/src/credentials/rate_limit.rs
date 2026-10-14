use std::time::{Duration, Instant};

/// How often exchanges are let through once a burst is spent: ten a second
/// (spec: LA-17).
const INTERVAL: Duration = Duration::from_millis(100);

/// How many exchanges may arrive at once before the rate applies (spec:
/// LA-17).
const BURST: u32 = 20;

/// The limit on exchanges across the whole host, as a generic cell rate: the
/// time by which the exchanges let through so far would have been spaced out
/// at [`INTERVAL`], allowed to run ahead of now by one burst.
///
/// One per host rather than one per caller, because every caller here is on
/// the same loopback address and there is nothing to tell them apart by.
pub(super) struct RateLimit {
    /// When the next exchange would be due if every one so far had been
    /// spaced at [`INTERVAL`].
    due: Instant,
}

impl RateLimit {
    pub(super) fn new(now: Instant) -> Self {
        Self { due: now }
    }

    /// Lets one exchange through, or answers how long until one would be.
    pub(super) fn take(&mut self, now: Instant) -> Result<(), Duration> {
        let due = self.due.max(now);
        let ahead = due - now;
        let tolerance = INTERVAL * (BURST - 1);
        if ahead > tolerance {
            return Err(ahead - tolerance);
        }
        self.due = due + INTERVAL;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // LA-17: a burst of 20 is let through at once, the 21st is told when the
    // next will be, and that one is let through when that time comes.
    #[test]
    fn a_burst_is_let_through_and_the_rate_applies_after_it() {
        let start = Instant::now();
        let mut limit = RateLimit::new(start);
        for _ in 0..BURST {
            assert_eq!(limit.take(start), Ok(()));
        }
        assert_eq!(limit.take(start), Err(INTERVAL));
        assert_eq!(limit.take(start + INTERVAL / 2), Err(INTERVAL / 2));
        assert_eq!(limit.take(start + INTERVAL), Ok(()));
        assert_eq!(limit.take(start + INTERVAL), Err(INTERVAL));
    }

    // LA-17: a host left alone long enough has its whole burst back, and no
    // more than that.
    #[test]
    fn an_idle_host_recovers_its_burst_and_no_more() {
        let start = Instant::now();
        let mut limit = RateLimit::new(start);
        for _ in 0..BURST {
            limit.take(start).unwrap();
        }
        let later = start + Duration::from_secs(60);
        for _ in 0..BURST {
            assert_eq!(limit.take(later), Ok(()));
        }
        assert!(limit.take(later).is_err());
    }
}
