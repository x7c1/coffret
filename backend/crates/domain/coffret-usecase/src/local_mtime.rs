use std::fs::Metadata;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use coffret_model::Mtime;

/// A local file's modification time, as the value an Entry carries
/// (spec: FM-9).
///
/// A clock that reports a moment before 1970 is recorded as one rather than
/// clamped, for the reason [`Mtime`] admits those at all: refusing the value
/// would lose the file's own time instead of correcting it. A filesystem that
/// keeps no modification time at all leaves the epoch, which is the only answer
/// available and is not evidence about the file.
pub(crate) fn mtime_of(metadata: &Metadata) -> Mtime {
    let Ok(modified) = metadata.modified() else {
        return Mtime::from_unix_seconds(0);
    };
    Mtime::from_unix_seconds(match modified.duration_since(UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_secs())
            .map(|seconds| -seconds)
            .unwrap_or(i64::MIN),
    })
}

/// The same time in the form a filesystem is handed it, or `None` where this
/// platform's clock cannot reach it.
///
/// The way back, for a fetch stamping a file it placed with the time its Entry
/// records (spec: EP-11). `None` rather than a clamp: a file stamped with a
/// time that is not its Entry's would look modified to the very next scan, so a
/// time that cannot be set is reported instead of approximated.
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
