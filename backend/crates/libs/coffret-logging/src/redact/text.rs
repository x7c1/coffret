use super::without_bearer::without_bearer;
use super::without_field::without_secret_fields;
use super::without_private::without_private;
use super::MAX_BODY_BYTES;

/// What is left in place of the tail of an over-long body.
pub(super) const ELIDED: &str = "…[elided]";

/// Takes the credentials out of a string and caps its length.
pub fn text(text: &str) -> String {
    let redacted = without_secret_fields(&flatten(text));
    elide(&without_bearer(&redacted))
}

/// Takes one caller-owned value out as well as credentials, then caps the
/// result.
///
/// Provider diagnostics ordinarily remain useful evidence, but a provider can
/// echo request data in its prose. A gateway uses this form when that request
/// data is a private location such as a configured Storage prefix. Empty input
/// means there is no private location to remove.
pub fn text_without(diagnostic: &str, private: &str) -> String {
    text(&without_private(diagnostic, private))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_private_value_echoed_by_a_provider_is_removed_with_credentials() {
        let prefix = "people/alice/Summer Library";
        let diagnostic = format!("could not list {prefix}/: Authorization: Bearer provider-token");

        let safe = text_without(&diagnostic, prefix);

        assert_eq!(
            safe,
            "could not list [redacted]/: Authorization: Bearer [redacted]"
        );
    }

    #[test]
    fn an_empty_private_value_removes_nothing() {
        assert_eq!(text_without("Storage answered", ""), "Storage answered");
    }
}
