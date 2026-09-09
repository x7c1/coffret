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
pub(crate) fn mtime_from_raw(seconds: impl TryInto<i64>, nanoseconds: impl TryInto<u64>) -> Mtime {
    Mtime::from_unix_seconds(whole_seconds(seconds, nanoseconds))
}

/// A local file's birth time, where the platform reports one (spec: FM-9).
///
/// Called only where the platform says the birth-time field is present. A
/// listing records `None` where it is not; the epoch would otherwise read as a
/// creation in 1970 that this device never observed.
///
/// The one moment it can be read is the one this is called at: unlike a name,
/// a birth time cannot be recovered once the local file is gone, and no fetch
/// stamps it onto the file it places (spec: EP-11).
///
/// A moment before 1970 is kept as one, for the reason
/// [`mtime_from_raw`] keeps one.
pub(crate) fn btime_from_raw(seconds: impl TryInto<i64>, nanoseconds: impl TryInto<u64>) -> Btime {
    Btime::from_unix_seconds(whole_seconds(seconds, nanoseconds))
}

/// One moment a filesystem reported, as whole seconds from the Unix epoch.
///
/// Shared by both times a listing reads, so the two cannot come to disagree
/// about what a moment before 1970 is or where a clock past what an `i64` holds
/// lands. Saturating rather than failing: a time a platform reports and this
/// type cannot hold is at the far end of what any clock states, and dropping
/// the file over it would lose the file rather than the second.
fn whole_seconds(raw_seconds: impl TryInto<i64>, raw_nanoseconds: impl TryInto<u64>) -> i64 {
    let seconds = raw_seconds.try_into().unwrap_or(i64::MAX);
    if seconds < 0 && raw_nanoseconds.try_into().is_ok_and(|part| part != 0) {
        // `SystemTime::duration_since` measured the magnitude before truncating
        // it to whole seconds. For example, -0.5 seconds therefore became zero,
        // while a raw `timespec` spells that instant as (-1, 500_000_000).
        seconds + 1
    } else {
        seconds
    }
}

/// One moment an Entry records, in the form a filesystem is handed it back, or
/// `None` where this platform's clock cannot reach it.
///
/// The way back from the two above, for a fetch stamping a file it placed with
/// the time its Entry records (spec: EP-11). `None` rather than a clamp: a file
/// stamped with a time that is not its Entry's would look modified to the very
/// next scan, so a time that cannot be set is reported instead of approximated.
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

    #[test]
    fn raw_times_before_the_epoch_and_past_the_domain_keep_the_existing_policy() {
        assert_eq!(
            mtime_from_raw(-86_400_i64, 0_u32).as_unix_seconds(),
            -86_400
        );
        assert_eq!(mtime_from_raw(u128::MAX, 0_u32).as_unix_seconds(), i64::MAX,);
        assert_eq!(
            btime_from_raw(-86_400_i64, 0_u32).as_unix_seconds(),
            -86_400,
        );
    }

    #[test]
    fn a_fractional_time_before_the_epoch_truncates_toward_zero() {
        assert_eq!(mtime_from_raw(-1_i64, 500_000_000_u32).as_unix_seconds(), 0,);
        assert_eq!(
            btime_from_raw(-2_i64, 500_000_000_u32).as_unix_seconds(),
            -1,
        );
    }
}
