use coffret_model::ContainerId;

use crate::chunk_size::ChunkSize;
use crate::error::{Error, Result};

/// The 32 plaintext bytes every Container starts with.
///
/// ```text
/// offset  size  field
/// ------  ----  -----
/// 0       5     magic = "CFRT1"
/// 5       1     format version = 0x01
/// 6       2     reserved = 0x0000
/// 8       16    Container ID
/// 24      4     chunk size (plaintext bytes per chunk)
/// 28      4     meta section length M (padded ciphertext bytes)
/// ```
///
/// The header carries no key material — Key Envelopes live in the Keyring — so
/// rotating the Master Key leaves every Container byte-for-byte unchanged. The
/// whole 32 bytes are the associated data of every AEAD message in the object,
/// which is what binds the meta section and the chunks to this exact header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Identifies the Container and names it on Storage.
    pub container_id: ContainerId,
    /// Plaintext bytes per chunk, honored by readers as recorded.
    pub chunk_size: ChunkSize,
    /// Length of the encrypted meta section in bytes, tag included.
    pub meta_len: u32,
}

impl Header {
    /// Total length of the header in bytes.
    pub const LEN: usize = 32;

    /// Length of the magic in bytes.
    pub const MAGIC_LEN: usize = 5;

    /// The bytes every Container v1 object starts with.
    pub const MAGIC: [u8; Self::MAGIC_LEN] = *b"CFRT1";

    /// The format version this crate writes and reads.
    pub const VERSION: u8 = 0x01;

    const VERSION_OFFSET: usize = 5;
    const RESERVED_RANGE: std::ops::Range<usize> = 6..8;
    const CONTAINER_ID_RANGE: std::ops::Range<usize> = 8..24;
    const CHUNK_SIZE_RANGE: std::ops::Range<usize> = 24..28;
    const META_LEN_RANGE: std::ops::Range<usize> = 28..32;

    /// Serializes the header. Multi-byte integers are big-endian.
    pub fn to_bytes(&self) -> [u8; Self::LEN] {
        let mut bytes = [0u8; Self::LEN];
        bytes[..Self::MAGIC_LEN].copy_from_slice(&Self::MAGIC);
        bytes[Self::VERSION_OFFSET] = Self::VERSION;
        bytes[Self::CONTAINER_ID_RANGE].copy_from_slice(self.container_id.as_bytes());
        bytes[Self::CHUNK_SIZE_RANGE].copy_from_slice(&self.chunk_size.get().to_be_bytes());
        bytes[Self::META_LEN_RANGE].copy_from_slice(&self.meta_len.to_be_bytes());
        bytes
    }

    /// Reads the header off the front of an object.
    ///
    /// Every check here is on plaintext bytes, so an object that is not a
    /// Container v1 is rejected without a key ever being used.
    pub fn parse(object: &[u8]) -> Result<Self> {
        let bytes = object.get(..Self::LEN).ok_or(Error::HeaderTooShort {
            actual: object.len(),
        })?;
        let magic: [u8; Self::MAGIC_LEN] = bytes[..Self::MAGIC_LEN]
            .try_into()
            .expect("the slice is MAGIC_LEN long");
        if magic != Self::MAGIC {
            return Err(Error::UnknownMagic { actual: magic });
        }
        let version = bytes[Self::VERSION_OFFSET];
        if version != Self::VERSION {
            return Err(Error::UnsupportedVersion { actual: version });
        }
        if bytes[Self::RESERVED_RANGE].iter().any(|byte| *byte != 0) {
            return Err(Error::ReservedNotZero);
        }
        let container_id = ContainerId::from_bytes(
            bytes[Self::CONTAINER_ID_RANGE]
                .try_into()
                .expect("the slice is ContainerId::BYTE_LEN long"),
        );
        let chunk_size = ChunkSize::new(u32::from_be_bytes(
            bytes[Self::CHUNK_SIZE_RANGE]
                .try_into()
                .expect("the slice is 4 bytes long"),
        ))?;
        let meta_len = u32::from_be_bytes(
            bytes[Self::META_LEN_RANGE]
                .try_into()
                .expect("the slice is 4 bytes long"),
        );
        Ok(Self {
            container_id,
            chunk_size,
            meta_len,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Header {
        Header {
            container_id: ContainerId::from_bytes([9u8; ContainerId::BYTE_LEN]),
            chunk_size: ChunkSize::DEFAULT,
            meta_len: 1234,
        }
    }

    #[test]
    fn round_trips_through_bytes() {
        let header = sample();
        assert_eq!(Header::parse(&header.to_bytes()), Ok(header));
    }

    // FM-2: the header is magic "CFRT1", format version 0x01, two reserved
    // bytes, the Container ID, the chunk size, and the meta section length, at
    // those exact offsets, with multi-byte integers big-endian.
    #[test]
    fn layout_matches_the_field_table() {
        let bytes = sample().to_bytes();
        assert_eq!(bytes.len(), 32);
        assert_eq!(&bytes[..5], b"CFRT1");
        assert_eq!(bytes[5], 0x01);
        assert_eq!(&bytes[6..8], &[0x00, 0x00]);
        assert_eq!(&bytes[8..24], &[9u8; 16]);
        assert_eq!(&bytes[24..28], &(1024u32 * 1024).to_be_bytes());
        assert_eq!(&bytes[28..32], &1234u32.to_be_bytes());
    }

    #[test]
    fn short_input_is_rejected() {
        assert_eq!(
            Header::parse(b"CFRT1"),
            Err(Error::HeaderTooShort { actual: 5 })
        );
    }

    // FM-2: reserved bytes must be zero.
    #[test]
    fn reserved_bytes_must_be_zero() {
        let mut bytes = sample().to_bytes();
        bytes[6] = 0x01;
        assert_eq!(Header::parse(&bytes), Err(Error::ReservedNotZero));
    }

    #[test]
    fn zero_chunk_size_is_rejected() {
        let mut bytes = sample().to_bytes();
        bytes[24..28].copy_from_slice(&0u32.to_be_bytes());
        assert_eq!(Header::parse(&bytes), Err(Error::InvalidChunkSize));
    }
}
