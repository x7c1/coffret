use super::super::REDACTED;

/// Replaces `private` only where it stands as a token of its own.
///
/// See the module documentation of [`redact`](crate::redact) for why an
/// occurrence inside a longer run of characters is left alone.
pub(super) fn without_token(text: &str, private: &str) -> String {
    debug_assert!(!private.is_empty(), "an empty value matches everywhere");

    let mut safe = String::with_capacity(text.len());
    let mut cursor = 0;
    while let Some(offset) = text[cursor..].find(private) {
        let start = cursor + offset;
        let end = start + private.len();
        if stands_alone(text, start, end) {
            safe.push_str(&text[cursor..start]);
            safe.push_str(REDACTED);
            cursor = end;
            continue;
        }
        // Not this occurrence, but a later one may still stand alone — a
        // provider is free to name the value twice in one sentence.
        let step = text[start..].chars().next().map_or(1, char::len_utf8);
        safe.push_str(&text[cursor..start + step]);
        cursor = start + step;
    }
    safe.push_str(&text[cursor..]);

    safe
}

/// Whether what occupies `start..end` is bounded on both sides.
fn stands_alone(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    before.is_none_or(bounds_a_location) && after.is_none_or(bounds_a_location)
}

/// Whether a character cannot be part of a bucket name or a path segment, and
/// therefore marks where one ends.
///
/// `-`, `.` and `_` are deliberately absent: all three are ordinary inside a
/// bucket name and inside a key, so treating them as edges would let a
/// provider's `my-bucket-backup` hide the configured `my-bucket`.
fn bounds_a_location(character: char) -> bool {
    character.is_whitespace()
        || character.is_control()
        || matches!(
            character,
            '/' | '\\'
                | '"'
                | '\''
                | '<'
                | '>'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | ','
                | ';'
                | ':'
                | '='
                | '?'
                | '&'
                | '%'
                | '+'
                | '*'
                | '|'
                | '@'
                | '!'
                | '#'
                | '$'
                | '^'
                | '~'
                | '`'
        )
}
