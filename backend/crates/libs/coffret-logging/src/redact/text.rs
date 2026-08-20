use super::without_bearer::without_bearer;
use super::without_field::without_secret_fields;
use super::MAX_BODY_BYTES;

/// What is left in place of the tail of an over-long body.
pub(super) const ELIDED: &str = "…[elided]";

/// Takes the credentials out of a string and caps its length.
pub fn text(text: &str) -> String {
    let redacted = without_secret_fields(&flatten(text));
    elide(&without_bearer(&redacted))
}

/// Puts a body on one line.
///
/// One event is one line in the file, which is what lets the log be read with
/// the tools that read lines. A provider's XML arrives with newlines in it and
/// would otherwise split its own event in two, leaving half of it looking like
/// an event of its own.
fn flatten(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

/// Cuts a body down to what an event may carry.
fn elide(text: &str) -> String {
    if text.len() <= MAX_BODY_BYTES {
        return text.to_owned();
    }

    let mut end = MAX_BODY_BYTES - ELIDED.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{ELIDED}", &text[..end])
}
