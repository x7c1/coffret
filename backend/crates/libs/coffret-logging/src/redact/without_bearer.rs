use super::REDACTED;

/// The scheme whose value is a bearer credential wherever it appears.
const BEARER: &str = "bearer ";

/// Replaces whatever follows a `Bearer` scheme.
pub(super) fn without_bearer(text: &str) -> String {
    // Matching on a lowercased copy keeps the search case-insensitive; every
    // byte of it is ASCII in the parts that matter, so the offsets are the
    // offsets of the original.
    let lowered = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut from = 0;

    while let Some(at) = lowered[from..].find(BEARER) {
        let credential = from + at + BEARER.len();
        out.push_str(&text[from..credential]);
        out.push_str(REDACTED);
        from = credential
            + text[credential..]
                .find(['"', ' ', ',', '\n', '\r'])
                .unwrap_or(text.len() - credential);
    }

    out.push_str(&text[from..]);
    out
}
