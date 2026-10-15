use std::time::Instant;

use super::Clock;

/// The operating system's monotonic clock: what the host runs on.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}
