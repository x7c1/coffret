use crate::error::{Error, Result};

/// One Container Key, encrypted so that only the Master Key can open it.
///
/// The envelope is 72 bytes — `nonce(24) ‖ ciphertext(32) ‖ tag(16)` — and is
/// bound to the Container it belongs to, so an envelope presented for a
/// different Container fails to unwrap. Envelopes live in the Keyring, never in
/// a Container header, which is what lets Master Key rotation leave Containers
/// byte-for-byte unchanged.
///
/// The bytes are ciphertext, so unlike the keys themselves this type is
/// ordinary data: it compares, prints, and copies like any other identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEnvelope([u8; Self::BYTE_LEN]);

impl KeyEnvelope {
    /// Length of a Key Envelope in bytes.
    pub const BYTE_LEN: usize = 72;

    /// Takes 72 raw bytes.
    pub const fn from_bytes(bytes: [u8; Self::BYTE_LEN]) -> Self {
        Self(bytes)
    }

    /// Takes a slice that must be exactly 72 bytes long.
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let bytes: [u8; Self::BYTE_LEN] =
            bytes.try_into().map_err(|_| Error::InvalidByteLength {
                expected: Self::BYTE_LEN,
                actual: bytes.len(),
            })?;
        Ok(Self(bytes))
    }

    /// The raw 72 bytes.
    pub const fn as_bytes(&self) -> &[u8; Self::BYTE_LEN] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_slice_rejects_wrong_length() {
        let result = KeyEnvelope::from_slice(&[0u8; 71]);
        assert!(
            matches!(
                result,
                Err(Error::InvalidByteLength {
                    expected: 72,
                    actual: 71
                })
            ),
            "expected 72 bytes and found 71, got {result:?}"
        );
    }

    #[test]
    fn from_slice_accepts_exact_length() {
        let envelope = KeyEnvelope::from_slice(&[5u8; 72]).expect("72 bytes is a valid envelope");
        assert_eq!(envelope.as_bytes(), &[5u8; 72]);
    }
}
