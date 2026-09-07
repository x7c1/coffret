use std::time::{Duration, SystemTime, UNIX_EPOCH};

use coffret_model::Mtime;

/// One moment an Entry records, in the form a filesystem is handed it back, or
/// `None` where this platform's clock cannot reach it.
///
/// The way back, for a fetch stamping a file it placed with the time its Entry
/// records (spec: EP-11). `None` rather than a clamp: a file stamped with a
/// time that is not its Entry's would look modified to the very next scan, so a
/// time that cannot be set is reported instead of approximated.
///
/// The way *out* — a local file's modification and birth times, as the values an
/// Entry carries (spec: FM-9) — is the local filesystem gateway's, behind
/// [`MappedRoots`](crate::MappedRoots): reading them means holding a filesystem's
/// own metadata, and this crate calls no filesystem.
pub(crate) fn system_time_of(mtime: Mtime) -> Option<SystemTime> {
    let seconds = Duration::from_secs(mtime.as_unix_seconds().unsigned_abs());
    if mtime.as_unix_seconds() < 0 {
        UNIX_EPOCH.checked_sub(seconds)
    } else {
        UNIX_EPOCH.checked_add(seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_after_the_epoch_round_trips() {
        let stamped = system_time_of(Mtime::from_unix_seconds(1_700_000_000))
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
        let stamped = system_time_of(Mtime::from_unix_seconds(-86_400))
            .expect("a day before the epoch is representable");
        assert!(stamped < UNIX_EPOCH);
    }
}
