use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A modification time, as whole seconds from the Unix epoch.
///
/// Two records carry one, with different trust: an Entry's mtime is
/// encrypted metadata the Container preserves for its user file, while a
/// Storage listing reports one per stored object — the provider's own,
/// untrusted record about the ciphertext, not the Entry's time.
///
/// Negative values are legal and mean "before 1970" — a file can carry any
/// timestamp its filesystem allows, and rejecting some of them would lose
/// information the Container is supposed to preserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mtime(i64);

impl Mtime {
    /// Takes a count of seconds from the Unix epoch.
    pub const fn from_unix_seconds(seconds: i64) -> Self {
        Self(seconds)
    }

    /// The count of seconds from the Unix epoch.
    pub const fn as_unix_seconds(&self) -> i64 {
        self.0
    }

    /// Takes a count of milliseconds from the Unix epoch, keeping the whole
    /// second the moment falls in.
    ///
    /// For a time stated to the millisecond — what a browser knows of a file
    /// dropped onto the explorer. The second is the one the moment falls in, so
    /// the count is truncated toward negative infinity rather than toward zero:
    /// half a second before 1970 is in the last second of 1969, not the first
    /// of 1970, and a time before the epoch stays before it (spec: FM-9).
    pub const fn from_unix_millis(millis: i64) -> Self {
        Self(millis.div_euclid(1000))
    }

    /// The same moment in the form a filesystem is handed it, or `None` where
    /// this platform's clock cannot reach it.
    ///
    /// For a fetch stamping a file it placed with the time its Entry records
    /// (spec: EP-11). `None` rather than a clamp: a file stamped with a time
    /// that is not its Entry's would look modified to the very next scan, so a
    /// time that cannot be set is a refusal about the Entry rather than an
    /// approximation of it. Asked here, of the value, so that the refusal is
    /// the flow's to name before anything is handed to a filesystem.
    pub fn to_system_time(self) -> Option<SystemTime> {
        let seconds = Duration::from_secs(self.0.unsigned_abs());
        if self.0 < 0 {
            UNIX_EPOCH.checked_sub(seconds)
        } else {
            UNIX_EPOCH.checked_add(seconds)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_after_the_epoch_round_trips() {
        let stamped = Mtime::from_unix_seconds(1_700_000_000)
            .to_system_time()
            .expect("a time within this century is representable");
        assert_eq!(
            stamped
                .duration_since(UNIX_EPOCH)
                .expect("it is after the epoch")
                .as_secs(),
            1_700_000_000,
        );
    }

    // FM-9: an mtime is whole seconds, and a time stated to the millisecond is
    // in the second it falls in — after the epoch the fraction is dropped, and
    // before it the moment is in the earlier second, never moved past 1970.
    #[test]
    fn milliseconds_are_kept_as_the_second_they_fall_in() {
        for (millis, seconds) in [
            (1_444_000_000_999, 1_444_000_000),
            (1_000, 1),
            (999, 0),
            (0, 0),
            (-1, -1),
            (-999, -1),
            (-1_000, -1),
            (-1_001, -2),
            (-86_400_500, -86_401),
        ] {
            assert_eq!(
                Mtime::from_unix_millis(millis).as_unix_seconds(),
                seconds,
                "{millis} ms",
            );
        }
    }

    // FM-9: a file may carry any timestamp its filesystem allows, so a moment
    // before 1970 is a value to preserve rather than one to correct.
    #[test]
    fn a_time_before_the_epoch_stays_before_it() {
        let stamped = Mtime::from_unix_seconds(-86_400)
            .to_system_time()
            .expect("a day before the epoch is representable");
        assert!(stamped < UNIX_EPOCH);
    }
}
