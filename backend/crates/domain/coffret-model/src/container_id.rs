use crate::error::{Error, Result};
use std::fmt;

/// The 128-bit identifier a Container carries for its whole life.
///
/// The identifier is drawn from a CSPRNG and takes no input from the content it
/// names, which is what lets the Storage name derived from it say nothing about
/// what the object holds. Generation needs an entropy source and therefore
/// lives in `coffret-format`; this type only holds and formats the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContainerId([u8; Self::BYTE_LEN]);

impl ContainerId {
    /// Length of a Container ID in bytes.
    pub const BYTE_LEN: usize = 16;

    /// Length of a Container ID in hex characters.
    pub const HEX_LEN: usize = Self::BYTE_LEN * 2;

    /// Extension every coffret Storage Object name ends with.
    pub const STORAGE_EXTENSION: &'static str = ".cfrt";

    /// Wraps 16 raw bytes.
    pub const fn from_bytes(bytes: [u8; Self::BYTE_LEN]) -> Self {
        Self(bytes)
    }

    /// The raw 16 bytes.
    pub const fn as_bytes(&self) -> &[u8; Self::BYTE_LEN] {
        &self.0
    }

    /// Parses the 32-lowercase-hex-character spelling.
    pub fn from_hex(hex: &str) -> Result<Self> {
        if hex.len() != Self::HEX_LEN {
            return Err(Error::InvalidHexLength {
                expected: Self::HEX_LEN,
                actual: hex.chars().count(),
            });
        }
        let mut bytes = [0u8; Self::BYTE_LEN];
        for (byte, pair) in bytes.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
            *byte = hex_digit(pair[0])? << 4 | hex_digit(pair[1])?;
        }
        Ok(Self(bytes))
    }

    /// The 32-lowercase-hex-character spelling.
    pub fn to_hex(&self) -> String {
        let mut hex = String::with_capacity(Self::HEX_LEN);
        for byte in self.0 {
            hex.push(hex_char(byte >> 4));
            hex.push(hex_char(byte & 0x0f));
        }
        hex
    }

    /// The name this Container is stored under: the ID as 32 lowercase hex
    /// characters followed by `.cfrt`.
    pub fn storage_name(&self) -> String {
        let mut name = self.to_hex();
        name.push_str(Self::STORAGE_EXTENSION);
        name
    }
}

impl fmt::Display for ContainerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ContainerId {
        ContainerId::from_bytes([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ])
    }

    // FM-3: a Container's Storage name is its ID as 32 lowercase hex characters
    // followed by `.cfrt`, so the name says nothing about the content.
    #[test]
    fn storage_name_is_lowercase_hex_id_plus_extension() {
        // The sample carries every nibble value, so this one expected string
        // pins the 32-character length and the lowercase spelling of each digit.
        assert_eq!(
            sample().storage_name(),
            "00112233445566778899aabbccddeeff.cfrt"
        );
    }

    #[test]
    fn hex_round_trips() {
        let id = sample();
        assert_eq!(ContainerId::from_hex(&id.to_hex()), Ok(id));
    }

    #[test]
    fn from_hex_rejects_wrong_length() {
        assert_eq!(
            ContainerId::from_hex("00112233"),
            Err(Error::InvalidHexLength {
                expected: 32,
                actual: 8
            })
        );
    }

    #[test]
    fn from_hex_rejects_uppercase() {
        assert_eq!(
            ContainerId::from_hex("00112233445566778899AABBCCDDEEFF"),
            Err(Error::InvalidHexDigit { found: 'A' })
        );
    }
}
