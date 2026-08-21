use std::borrow::Cow;

/// What is left in place of the tail of an event too large to fit a file.
///
/// An event is cut down rather than dropped: an oversized record still says
/// which operation answered how, and losing its tail is a smaller loss than
/// losing the event — or than letting one record push the total past the
/// ceiling.
///
/// The file is JSONL, and half a JSON object is not one: whatever is left of
/// the record is a line no parser can read, and pretending otherwise would give
/// a reader a broken line with nothing to say why. So the cut ends that line,
/// and this follows it as a record of its own — valid JSON, shaped like every
/// other line, `level` and `target` and `fields` where they always are. A
/// reader that skips unparseable lines (`jq -R 'fromjson? // empty'`) therefore
/// still sees, in the same query, that a record was cut here rather than
/// silently losing one.
///
/// It carries no timestamp of its own because the line above it has one: what
/// the cut keeps is the head of the record, and the head is where the formatter
/// writes the time, the level, and the target.
pub(super) const TRUNCATION_MARKER: &[u8] = concat!(
    "\n{\"level\":\"WARN\",\"fields\":",
    "{\"message\":\"the unparseable line above is one record, cut to fit the file\",",
    "\"truncated\":true},\"target\":\"coffret_logging\"}\n"
)
.as_bytes();

/// Cuts a record down to what one file can hold.
pub(super) fn cap(record: &[u8], max_file_bytes: u64) -> Cow<'_, [u8]> {
    if record.len() as u64 <= max_file_bytes {
        return Cow::Borrowed(record);
    }

    // Saturating: a file size smaller than the marker keeps none of the record
    // and writes the marker alone, rather than wrapping into a cut past the end
    // of it. `LogSettings::sizes` holds a floor well above the marker, so this
    // is unreachable today — and not something this function should rely on.
    let keep = floor_char_boundary(
        record,
        (max_file_bytes as usize).saturating_sub(TRUNCATION_MARKER.len()),
    );
    let mut capped = Vec::with_capacity(keep + TRUNCATION_MARKER.len());
    capped.extend_from_slice(&record[..keep]);
    capped.extend_from_slice(TRUNCATION_MARKER);
    Cow::Owned(capped)
}

/// The largest cut no further than `at` that does not split a character.
///
/// A formatted event is UTF-8, and half a character in the file would make the
/// whole line unreadable to whatever comes to read it.
fn floor_char_boundary(bytes: &[u8], at: usize) -> usize {
    let mut at = at.min(bytes.len());
    // A continuation byte is `10xxxxxx`; anything else starts a character. A
    // cut at the very end splits nothing, so there is nothing to look at.
    while at > 0 && at < bytes.len() && bytes[at] & 0b1100_0000 == 0b1000_0000 {
        at -= 1;
    }
    at
}
