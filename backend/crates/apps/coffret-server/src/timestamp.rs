use coffret_device::{DepositedAt, Mtime};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// One modification time as ISO 8601 in UTC, or `None` where no clock reaches
/// it.
///
/// UTC and not the device's zone. The time belongs to the user's file and
/// travelled inside the Container that holds it (spec: FM-9), so it means the
/// same thing on every device that opens the Library; rendering it in the
/// server's zone would make the answer depend on where the server happens to be
/// rather than on what the Library holds. Which zone to show it in is the
/// browser's, and the browser knows the reader's.
///
/// `None` is a time no calendar can state. An mtime is a count of seconds and
/// may be any of them, before 1970 included, because a filesystem may carry any
/// (spec: FM-9) — and one far enough out is not a date. Saying so is better than
/// either refusing the row or naming a moment that is not the file's.
pub fn iso8601(mtime: Mtime) -> Option<String> {
    rfc3339(mtime.as_unix_seconds())
}

/// One deposit time the same way: in UTC, and `None` where no calendar reaches
/// it (spec: FM-15).
///
/// The same form as a modification time so a page reads both alike, and a
/// separate function because it is a separate axis — when the Library
/// received the content, not when the file was last changed.
pub fn deposit_iso8601(deposited_at: DepositedAt) -> Option<String> {
    rfc3339(deposited_at.as_unix_seconds())
}

fn rfc3339(seconds: i64) -> Option<String> {
    OffsetDateTime::from_unix_timestamp(seconds)
        .ok()?
        .format(&Rfc3339)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::{deposit_iso8601, iso8601};
    use coffret_device::{DepositedAt, Mtime};

    #[test]
    fn a_time_is_stated_in_utc() {
        assert_eq!(
            iso8601(Mtime::from_unix_seconds(1_700_000_000)).as_deref(),
            Some("2023-11-14T22:13:20Z"),
        );
    }

    // FM-9 preserves whatever the file carried, and a file may carry a time
    // before 1970. It is a date like any other.
    #[test]
    fn a_time_before_the_epoch_is_a_date_like_any_other() {
        assert_eq!(
            iso8601(Mtime::from_unix_seconds(-1)).as_deref(),
            Some("1969-12-31T23:59:59Z"),
        );
    }

    #[test]
    fn a_count_of_seconds_no_calendar_reaches_is_no_date() {
        assert_eq!(iso8601(Mtime::from_unix_seconds(i64::MAX)), None);
    }

    #[test]
    fn a_deposit_time_is_stated_in_utc() {
        assert_eq!(
            deposit_iso8601(DepositedAt::from_unix_seconds(1_760_000_000)).as_deref(),
            Some("2025-10-09T08:53:20Z"),
        );
        assert_eq!(
            deposit_iso8601(DepositedAt::from_unix_seconds(i64::MAX)),
            None
        );
    }
}
