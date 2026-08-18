use crate::error::{Error, Result};

/// The BLAKE3-256 hash of an Entry's plaintext.
///
/// It serves end-to-end verification after decryption and change detection
/// between revisions of the same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash([u8; Self::BYTE_LEN]);

impl ContentHash {
    /// Length of a content hash in bytes.
    pub const BYTE_LEN: usize = 32;

    /// Takes 32 raw bytes.
    pub const fn from_bytes(bytes: [u8; Self::BYTE_LEN]) -> Self {
        Self(bytes)
    }

    /// Takes a slice that must be exactly 32 bytes long.
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let bytes: [u8; Self::BYTE_LEN] =
            bytes.try_into().map_err(|_| Error::InvalidByteLength {
                expected: Self::BYTE_LEN,
                actual: bytes.len(),
            })?;
        Ok(Self(bytes))
    }

    /// The raw 32 bytes.
    pub const fn as_bytes(&self) -> &[u8; Self::BYTE_LEN] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_slice_rejects_wrong_length() {
        assert_eq!(
            ContentHash::from_slice(&[0u8; 31]),
            Err(Error::InvalidByteLength {
                expected: 32,
                actual: 31
            })
        );
    }

    #[test]
    fn from_slice_accepts_exact_length() {
        let hash = ContentHash::from_slice(&[7u8; 32]).expect("32 bytes is a valid hash");
        assert_eq!(hash.as_bytes(), &[7u8; 32]);
    }
}
