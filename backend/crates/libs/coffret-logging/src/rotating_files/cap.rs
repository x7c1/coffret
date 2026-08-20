use std::borrow::Cow;

/// What is left in place of the tail of an event too large to fit a file.
///
/// An event is cut down rather than dropped: an oversized record still says
/// which operation answered how, and losing its tail is a smaller loss than
/// losing the event — or than letting one record push the total past the
/// ceiling.
const TRUNCATION_MARKER: &[u8] = b"[record truncated]\n";

/// Cuts a record down to what one file can hold.
pub(super) fn cap(record: &[u8], max_file_bytes: u64) -> Cow<'_, [u8]> {
    if record.len() as u64 <= max_file_bytes {
        return Cow::Borrowed(record);
    }

    let keep = floor_char_boundary(record, max_file_bytes as usize - TRUNCATION_MARKER.len());
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
