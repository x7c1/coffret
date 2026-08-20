//! The hex spelling every byte string in a manifest travels in.
//!
//! Lowercase only, as every hex spelling in coffret is: two spellings of one
//! value would be two manifests describing one fixture set.

use anyhow::{bail, Result};

const DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Spells bytes as lowercase hex.
pub fn encode(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        hex.push(char::from(DIGITS[usize::from(byte >> 4)]));
        hex.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    hex
}

/// Reads a lowercase hex string of any even length.
pub fn decode(hex: &str) -> Result<Vec<u8>> {
    let (pairs, remainder) = hex.as_bytes().as_chunks::<2>();
    if !remainder.is_empty() {
        bail!(
            "expected an even number of hex characters, found {}",
            hex.len()
        );
    }
    pairs
        .iter()
        .map(|pair| Ok(digit(pair[0])? << 4 | digit(pair[1])?))
        .collect()
}

/// Reads a lowercase hex string that must spell exactly `N` bytes.
pub fn decode_array<const N: usize>(hex: &str) -> Result<[u8; N]> {
    let bytes = decode(hex)?;
    let len = bytes.len();
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected {N} bytes, found {len}"))
}

fn digit(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => bail!(
            "expected a lowercase hex character, found {:?}",
            char::from(byte)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        assert_eq!(encode(&[0x00, 0x0f, 0xa1, 0xff]), "000fa1ff");
        assert_eq!(decode("000fa1ff").unwrap(), vec![0x00, 0x0f, 0xa1, 0xff]);
    }

    #[test]
    fn uppercase_is_rejected() {
        assert!(decode("00FF").is_err());
    }

    #[test]
    fn an_odd_length_is_rejected() {
        assert!(decode("abc").is_err());
    }

    #[test]
    fn a_fixed_width_value_of_the_wrong_length_is_rejected() {
        assert!(decode_array::<2>("aabbcc").is_err());
        assert_eq!(decode_array::<2>("aabb").unwrap(), [0xaa, 0xbb]);
    }
}
