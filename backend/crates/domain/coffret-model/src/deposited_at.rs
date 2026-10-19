/// When the content now at an Entry Path was deposited into the Library — its
/// **deposit time** — as whole seconds from the Unix epoch (spec: FM-15).
///
/// A deposit is putting new content into the Library, and whether content is
/// new is decided by its hash: an addition is a deposit, and so is a
/// replacement whose content hashes differently from the Entry it replaces.
/// Each is stamped by the device that builds the Journal record carrying it,
/// with that device's own clock and no correction for skew between devices.
/// What puts no new content in carries the time forward instead — a rename, a
/// Pack rewrite, a freeze absorbing a one-file Entry it does not change, a
/// replacement (a key-lost Entry's included) with the content it replaces — so
/// a new Entry carrying an existing Entry's content keeps that Entry's deposit
/// time (spec: PK-10, PK-11, PK-13). A restore recovers it from the checkpoint
/// and the records exactly as it recovers the current path, and is no deposit
/// (spec: RV-1).
///
/// A catalog value, carried by a record's additions and a Snapshot's entries
/// and never by a Container's meta section: a Container records what was
/// captured when it was written, and a rewritten one would mix that with a
/// value the catalog holds now (spec: FM-9).
///
/// A separate axis from [`Mtime`](crate::Mtime) and [`Btime`](crate::Btime),
/// which describe the original file: a photograph taken years ago and
/// deposited today has an old modification time and a deposit time of today.
///
/// Negative values are representable: a device's clock may report one, and
/// refusing it would lose the record rather than fix it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DepositedAt(i64);

impl DepositedAt {
    /// Takes a count of seconds from the Unix epoch.
    pub const fn from_unix_seconds(seconds: i64) -> Self {
        Self(seconds)
    }

    /// The count of seconds from the Unix epoch.
    pub const fn as_unix_seconds(&self) -> i64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_round_trip_across_the_epoch() {
        for seconds in [i64::MIN, -1, 0, 1, 1_700_000_000, i64::MAX] {
            assert_eq!(
                DepositedAt::from_unix_seconds(seconds).as_unix_seconds(),
                seconds
            );
        }
    }
}
