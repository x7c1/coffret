use super::REDACTED;

/// The fields of an OAuth answer that are credentials rather than facts.
const SECRET_FIELDS: [&str; 5] = [
    "access_token",
    "refresh_token",
    "id_token",
    "client_secret",
    "code_verifier",
];

/// Replaces the value of every field that is a credential.
pub(super) fn without_secret_fields(text: &str) -> String {
    let mut redacted = text.to_owned();
    for field in SECRET_FIELDS {
        redacted = without_field(&redacted, field);
    }
    redacted
}

/// Replaces the value of one named field, however it is written.
///
/// JSON (`"access_token":"…"`) and form encoding (`access_token=…`) both
/// appear in what an OAuth endpoint sends and answers, so both are handled.
fn without_field(text: &str, field: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(at) = rest.find(field) {
        let (before, from_field) = rest.split_at(at + field.len());
        out.push_str(before);

        let Some(value) = value_after(from_field) else {
            rest = from_field;
            continue;
        };
        out.push_str(&from_field[..value.start]);
        out.push_str(REDACTED);
        rest = &from_field[value.end..];
    }

    out.push_str(rest);
    out
}

/// Where the value assigned to a field just named begins and ends.
///
/// `None` where the name was not being used to assign anything — a field named
/// inside an error message, say — in which case there is nothing to take out.
fn value_after(text: &str) -> Option<std::ops::Range<usize>> {
    let mut chars = text.char_indices().peekable();

    // The name may be quoted, and the separator may be either JSON's or a
    // form's; whitespace is allowed around it as JSON allows it.
    let mut assigned = false;
    let mut start = None;
    for (at, character) in chars.by_ref() {
        match character {
            '"' | ' ' | '\t' | '\n' | '\r' if !assigned => continue,
            ':' | '=' if !assigned => assigned = true,
            ' ' | '\t' | '\n' | '\r' if assigned => continue,
            _ if assigned => {
                start = Some((at, character));
                break;
            }
            _ => return None,
        }
    }

    let (start, first) = start?;
    // A quoted value runs to its closing quote; a form's runs to the separator
    // that ends the pair.
    let (from, terminators): (usize, &[char]) = match first {
        '"' => (start + 1, &['"']),
        _ => (start, &['&', ',', '}', ' ', '\n']),
    };
    let end = text[from..]
        .find(terminators)
        .map(|at| from + at)
        .unwrap_or(text.len());

    Some(from..end)
}
