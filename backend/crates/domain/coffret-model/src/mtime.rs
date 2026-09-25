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
