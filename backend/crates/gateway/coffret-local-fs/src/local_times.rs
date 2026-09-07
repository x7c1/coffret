use std::fs::Metadata;
use std::time::{SystemTime, UNIX_EPOCH};

use coffret_model::{Btime, Mtime};

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
/// stamps it onto the file it places (spec: EP-11).
///
/// A moment before 1970 is kept as one, for the reason [`mtime_of`] keeps one.
pub(crate) fn btime_of(metadata: &Metadata) -> Option<Btime> {
    Some(Btime::from_unix_seconds(unix_seconds(
        metadata.created().ok()?,
    )))
}

/// One moment a filesystem reported, as whole seconds from the Unix epoch.
///
/// Shared by both times a listing reads, so the two cannot come to disagree
/// about what a moment before 1970 is or where a clock past what an `i64` holds
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
