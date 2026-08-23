/// A moment this device wrote down, as whole seconds from the Unix epoch.
///
/// It is deliberately not [`Mtime`](coffret_model::Mtime): an Mtime is a file's
/// own modification time, preserved for the user and carried inside a Container
/// (spec: FM-9), while this is the device's own clock recording when it looked
/// at something or started something. Nothing in the Library's correctness
/// rests on it — no commit is ordered by it and no conflict is resolved by it
/// (spec: CP-7) — so a clock that jumps costs a device nothing but the
/// precision of its own bookkeeping.
///
/// Values before 1970 are representable for the same reason an Mtime's are: a
/// clock may report one, and refusing it would lose the record rather than fix
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceTime(i64);

impl DeviceTime {
    /// Takes a count of seconds from the Unix epoch.
    pub const fn from_unix_seconds(seconds: i64) -> Self {
        Self(seconds)
    }

    /// The count of seconds from the Unix epoch.
    pub const fn as_unix_seconds(self) -> i64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_round_trip_across_the_epoch() {
        for seconds in [i64::MIN, -1, 0, 1, i64::MAX] {
            assert_eq!(
                DeviceTime::from_unix_seconds(seconds).as_unix_seconds(),
                seconds
            );
        }
    }
}
