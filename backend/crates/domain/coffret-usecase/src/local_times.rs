use std::fs::Metadata;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use coffret_model::{Btime, Mtime};

/// A local file's modification time, as the value an Entry carries
/// (spec: FM-9).
///
/// A clock that reports a moment before 1970 is recorded as one rather than
/// clamped, for the reason [`Mtime`] admits those at all: refusing the value
/// would lose the file's own time instead of correcting it. A filesystem that
/// keeps no modification time at all leaves the epoch, which is the only answer
/// available and is not evidence about the file.
pub fn mtime_of(metadata: &Metadata) -> Mtime {
    let Ok(modified) = metadata.modified() else {
        return Mtime::from_unix_seconds(0);
    };
    Mtime::from_unix_seconds(unix_seconds(modified))
}

/// A local file's birth time, where the platform reports one (spec: FM-9).
///
/// `None` is the whole answer where it does not: not every platform and
/// filesystem keeps a creation time, and `created()` says so with an `Err`. An
/// Entry written from such a file records no birth time at all rather than a
/// stand-in — the epoch would read as "created in 1970", and a modification
/// time would read as a creation this device never observed.
///
/// The one moment it can be read is the one this is called at: unlike a name,
/// a birth time cannot be recovered once the local file is gone, and no fetch
/// stamps it onto the file it places — `system_time_of` below is the way back
/// for the modification time alone (spec: EP-11).
///
/// A moment before 1970 is kept as one, for the reason [`mtime_of`] keeps one.
pub fn btime_of(metadata: &Metadata) -> Option<Btime> {
    Some(Btime::from_unix_seconds(unix_seconds(
        metadata.created().ok()?,
    )))
}

/// One moment a filesystem reported, as whole seconds from the Unix epoch.
///
/// Shared by both times a scan reads, so the two cannot come to disagree about
/// what a moment before 1970 is or where a clock past what an `i64` holds
/// lands. Saturating rather than failing: a time a platform reports and this
/// type cannot hold is at the far end of what any clock states, and dropping
/// the file over it would lose the file rather than the second.
fn unix_seconds(moment: SystemTime) -> i64 {
    match moment.duration_since(UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_secs())
            .map(|seconds| -seconds)
            .unwrap_or(i64::MIN),
    }
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
