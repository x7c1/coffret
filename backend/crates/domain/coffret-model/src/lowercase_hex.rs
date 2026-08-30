use crate::error::{Error, Result};

/// Whether `value` is a non-empty run of lowercase hex digits.
///
/// Every hex spelling in coffret is lowercase, because two spellings of one
/// value would be two names for one thing — a Container's object name (FM-3),
/// or a Keyring's `set_digest` both in a replica name and in the commitment a
/// Journal record selects that replica set with (FM-12, KL-3).
pub(crate) fn is_nonempty_lowercase_hex(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Spells `N` bytes as `2 * N` lowercase hex characters.
///
/// One spelling for every identifier that carries one — a Container ID (FM-3),
/// a Library ID (FM-18) — so a name built out of bytes is built the same way
/// wherever it is built.
pub(crate) fn encode<const N: usize>(bytes: &[u8; N]) -> String {
    let mut hex = String::with_capacity(N * 2);
    for byte in bytes {
        hex.push(hex_char(byte >> 4));
        hex.push(hex_char(byte & 0x0f));
    }
    hex
}

/// Reads `N` bytes back from exactly `2 * N` lowercase hex characters.
///
/// The inverse of [`encode`], and as strict: uppercase is a hex digit nowhere
/// in coffret, so an identifier spelled with one is refused rather than taken
/// as a second spelling of the same value.
pub(crate) fn decode<const N: usize>(hex: &str) -> Result<[u8; N]> {
    if hex.len() != N * 2 {
        return Err(Error::InvalidHexLength {
            expected: N * 2,
            // Counted in characters rather than in bytes: a caller is told a
            // length it can see in the text it passed.
            actual: hex.chars().count(),
        });
    }
    let mut bytes = [0u8; N];
    let (pairs, _) = hex.as_bytes().as_chunks::<2>();
    for (byte, pair) in bytes.iter_mut().zip(pairs) {
        *byte = hex_digit(pair[0])? << 4 | hex_digit(pair[1])?;
    }
    Ok(bytes)
}

fn hex_char(nibble: u8) -> char {
    char::from(b"0123456789abcdef"[usize::from(nibble)])
}

fn hex_digit(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(Error::InvalidHexDigit {
            found: char::from(byte),
        }),
    }
}
