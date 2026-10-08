use coffret_device::Mtime;

/// The modification time one part says its file has, or `None` where it says
/// none this route can read.
///
/// It travels as the part's field name: the whole of it, as a count of
/// milliseconds from the Unix epoch in decimal — what a browser knows of a
/// file as `File.lastModified`, and negative before 1970. The name is the one
/// thing about a part that is read before its bytes are, which is what the time
/// has to be, since the file is stamped with it before it is renamed into place
/// (spec: EP-11). Nothing else on the route reads the name, so carrying the time
/// in it costs a drop no second part per file against the budget it is counted
/// in (spec: LA-9).
///
/// A part whose name is anything else — none at all, or the `file` a caller
/// that knows nothing of times sends — is a file whose time nobody said, and
/// it keeps the one it is written at. That is not a refusal: the time is a
/// fact the browser happens to know, and a file is no less the person's for
/// arriving without it.
///
/// What is kept is the second the moment falls in (spec: FM-9), which is the
/// form a scan reads a time off a file in; [`Mtime::from_unix_millis`] says
/// which second that is before 1970.
pub(super) fn modified(field_name: Option<&str>) -> Option<Mtime> {
    let millis: i64 = field_name?.parse().ok()?;
    Some(Mtime::from_unix_millis(millis))
}

#[cfg(test)]
mod tests {
    use super::*;

    // EP-11, FM-9: the name is a count of milliseconds, and it is kept as the
    // second it falls in — before 1970 the earlier one.
    #[test]
    fn a_count_of_milliseconds_is_the_time_and_anything_else_is_none() {
        assert_eq!(
            modified(Some("1444000000999")),
            Some(Mtime::from_unix_seconds(1_444_000_000)),
        );
        assert_eq!(modified(Some("-1500")), Some(Mtime::from_unix_seconds(-2)));
        assert_eq!(modified(Some("0")), Some(Mtime::from_unix_seconds(0)));
        for unread in [
            None,
            Some("file"),
            Some(""),
            Some("1.5"),
            Some("99999999999999999999"),
        ] {
            assert_eq!(modified(unread), None, "{unread:?}");
        }
    }
}
