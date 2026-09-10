/// Produces the common URI representations in which an HTTP client or
/// provider can repeat a request value. Some render paths preserve `/`, while
/// query-style render paths use `+` for spaces.
pub(super) fn percent_encode(
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
