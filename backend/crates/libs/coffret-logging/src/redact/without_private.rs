use super::REDACTED;

/// Replaces one caller-owned private value wherever it appears.
///
/// A provider echoes what it was asked for, and the value as the caller knows
/// it is not the only spelling that can come back: an HTTP client may have
/// percent-encoded it on the way out, so every representation the request could
/// have carried is taken out too. An empty value means there is nothing to
/// remove.
pub(super) fn without_private(text: &str, private: &str) -> String {
    if private.is_empty() {
        return text.to_owned();
    }

    let mut safe = text.replace(private, REDACTED);
    for preserve_slash in [false, true] {
        for space_as_plus in [false, true] {
            for uppercase_hex in [false, true] {
                let encoded = percent_encode(private, preserve_slash, space_as_plus, uppercase_hex);
                if encoded != private {
                    safe = safe.replace(&encoded, REDACTED);
                }
            }
        }
    }

    safe
}

/// Produces the common URI representations in which an HTTP client or
/// provider can repeat a request value. Some render paths preserve `/`, while
/// query-style render paths use `+` for spaces.
fn percent_encode(
    value: &str,
    preserve_slash: bool,
    space_as_plus: bool,
    uppercase_hex: bool,
) -> String {
    const UPPER: &[u8; 16] = b"0123456789ABCDEF";
    const LOWER: &[u8; 16] = b"0123456789abcdef";
    let hex = if uppercase_hex { UPPER } else { LOWER };
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'~')
            || (preserve_slash && byte == b'/')
        {
            encoded.push(char::from(byte));
        } else if space_as_plus && byte == b' ' {
            encoded.push('+');
        } else {
            encoded.push('%');
            encoded.push(char::from(hex[usize::from(byte >> 4)]));
            encoded.push(char::from(hex[usize::from(byte & 0x0f)]));
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uri_encoded_private_value_is_removed() {
        let prefix = "people/alice/Summer Library";

        for text in [
            "URI=/people/alice/Summer%20Library/entry",
            "query=people%2falice%2fSummer+Library",
        ] {
            let safe = without_private(text, prefix);

            assert!(!safe.contains("alice"), "{safe}");
            assert!(safe.contains(REDACTED), "{safe}");
        }
    }
}
