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
///
/// Those 32 bytes are also the only part of a Container a reader believes before
/// it has authenticated anything, so the one length among them is held against
/// [`Header::MAX_META_LEN`] as it is read.
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

    /// The longest meta section this build reads or writes, tag included
    /// (spec: FM-2, FM-9).
    ///
    /// `meta_len` is 32 bits of *unauthenticated* plaintext. It is read before a
    /// key is used at all, and everything a reader does next is sized by it: the
    /// buffer the section is collected into, and — for a reader working in
    /// ranges — the second request it issues. A reader that took the field at
    /// its word would let anyone who edited four bytes of a stored object, or a
    /// provider answering for one, command an allocation of nearly 4 GiB and a
    /// range read to match, and would find out only afterwards that none of it
    /// authenticated. Authentication settles what the bytes *are*; it never
    /// bounds what obtaining them costs, so the bound is stated here.
    ///
    /// 64 MiB bounds the absurd rather than the ordinary. One meta section is
    /// one Container's entry table (spec: FM-9), and one row of it costs on the
    /// order of 120 bytes with the Entry Paths a real Library carries — the
    /// figure FM-16's Snapshot schema is sized against — so this admits a single
    /// Container of roughly half a million Entries, where a `freeze` of user
    /// media produces hundreds. A Pack of the photographs and book pages the
    /// format was shaped for carries a meta section of a few tens of kilobytes.
    ///
    /// What keeps an honest writer away from it is not the Pack size target but
    /// segmentation's own rule about the table. The target counts content and
    /// table together (spec: PK-3, PK-6), so a target measured in gigabytes
    /// filled with kilobyte-scale files would reach a table of tens of megabytes
    /// while the Pack was still well under target — the freeze would then be
    /// refused at layout, and cutting it again would cut it the same way. So
    /// segmentation closes a Pack once its entry table reaches half this
    /// ceiling, whatever the target says, and a freeze of very many small files
    /// produces more Packs instead of one Container that cannot be laid out.
    ///
    /// It binds the writer too — the layout a Container is planned from holds
    /// its own meta section against it — so every Container this build writes is
    /// one it will read back, and one that would need a larger table is refused
    /// while it is being laid out rather than stored unreadable. Raising it is a
    /// format decision, the kind a version that admits larger entry tables makes
    /// along with the rest of its rule, and never a transport knob tuned per
    /// provider.
    pub const MAX_META_LEN: u32 = 64 * 1024 * 1024;

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
    /// Container v1 is rejected without a key ever being used. The declared meta
    /// section length is one of them: it is held against
    /// [`MAX_META_LEN`](Self::MAX_META_LEN) here, which is before any caller has
    /// sized a buffer or a range request by it. A length past that ceiling is
    /// evidence of a tampered or substituted object exactly as a wrong magic is
    /// — nothing this build wrote declares one — and it is refused on the same
    /// terms, for nothing but the four bytes it took to read.
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
        if meta_len > Self::MAX_META_LEN {
            return Err(Error::MetaSectionTooLong {
                declared: u64::from(meta_len),
                limit: u64::from(Self::MAX_META_LEN),
            });
        }
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
        assert_eq!(
            Header::parse(&header.to_bytes()).expect("a header's own bytes parse back"),
            header
        );
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
        let result = Header::parse(b"CFRT1");
        assert!(
            matches!(result, Err(Error::HeaderTooShort { actual: 5 })),
            "expected 5 bytes to be too short for a header, got {result:?}"
        );
    }

    // FM-2: reserved bytes must be zero.
    #[test]
    fn reserved_bytes_must_be_zero() {
        let mut bytes = sample().to_bytes();
        bytes[6] = 0x01;
        let result = Header::parse(&bytes);
        assert!(
            matches!(result, Err(Error::ReservedNotZero)),
            "expected a non-zero reserved byte to be rejected, got {result:?}"
        );
    }

    #[test]
    fn zero_chunk_size_is_rejected() {
        let mut bytes = sample().to_bytes();
        bytes[24..28].copy_from_slice(&0u32.to_be_bytes());
        let result = Header::parse(&bytes);
        assert!(
            matches!(result, Err(Error::InvalidChunkSize)),
            "expected a chunk size of zero to be rejected, got {result:?}"
        );
    }
}
