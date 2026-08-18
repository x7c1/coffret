use coffret_model::{ControlObjectKind, Generation, ReplicaPosition};

use crate::error::{Error, Result};
use crate::nonce;

/// The 44 plaintext bytes every control object starts with.
///
/// ```text
/// offset  size  field
/// ------  ----  -----
/// 0       5     magic = "CFCTL"
/// 5       1     format version = 0x01
/// 6       1     kind (0x01 Journal / 0x02 Keyring / 0x03 Index Snapshot)
/// 7       1     reserved = 0x00
/// 8       8     generation
/// 16      2     replica index (0-based)
/// 18      2     replica count
/// 20      24    nonce (random)
/// ```
///
/// The whole 44 bytes are the associated data of the payload, so editing the
/// kind, the generation, the replica position, or the nonce fails
/// authentication. Unlike a Container, a control object carries its nonce: its
/// purpose key covers every object of that kind, so there is no per-object
/// counter to build a deterministic nonce from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlHeader {
    /// Which kind of control state the payload carries.
    pub kind: ControlObjectKind,
    /// How many times this kind has been rewritten within the epoch.
    pub generation: Generation,
    /// Which replica this is, out of how many.
    pub replica: ReplicaPosition,
    nonce: [u8; nonce::LEN],
}

/// The kind byte of a Journal record.
const KIND_JOURNAL: u8 = 0x01;
/// The kind byte of a Keyring replica.
const KIND_KEYRING: u8 = 0x02;
/// The kind byte of an Index Snapshot.
const KIND_INDEX_SNAPSHOT: u8 = 0x03;

impl ControlHeader {
    /// Total length of the header in bytes.
    pub const LEN: usize = 44;

    /// Length of the magic in bytes.
    pub const MAGIC_LEN: usize = 5;

    /// The bytes every control object v1 starts with.
    pub const MAGIC: [u8; Self::MAGIC_LEN] = *b"CFCTL";

    /// The format version this crate writes and reads.
    pub const VERSION: u8 = 0x01;

    const VERSION_OFFSET: usize = 5;
    const KIND_OFFSET: usize = 6;
    const RESERVED_OFFSET: usize = 7;
    const GENERATION_RANGE: std::ops::Range<usize> = 8..16;
    const REPLICA_INDEX_RANGE: std::ops::Range<usize> = 16..18;
    const REPLICA_COUNT_RANGE: std::ops::Range<usize> = 18..20;
    const NONCE_RANGE: std::ops::Range<usize> = 20..44;

    pub(crate) const fn new(
        kind: ControlObjectKind,
        generation: Generation,
        replica: ReplicaPosition,
        nonce: [u8; nonce::LEN],
    ) -> Self {
        Self {
            kind,
            generation,
            replica,
            nonce,
        }
    }

    /// The nonce the payload was encrypted under.
    pub(crate) const fn nonce(&self) -> &[u8; nonce::LEN] {
        &self.nonce
    }

    /// Serializes the header. Multi-byte integers are big-endian.
    pub fn to_bytes(&self) -> [u8; Self::LEN] {
        let mut bytes = [0u8; Self::LEN];
        bytes[..Self::MAGIC_LEN].copy_from_slice(&Self::MAGIC);
        bytes[Self::VERSION_OFFSET] = Self::VERSION;
        bytes[Self::KIND_OFFSET] = kind_byte(self.kind);
        bytes[Self::GENERATION_RANGE].copy_from_slice(&self.generation.get().to_be_bytes());
        bytes[Self::REPLICA_INDEX_RANGE].copy_from_slice(&self.replica.index().to_be_bytes());
        bytes[Self::REPLICA_COUNT_RANGE].copy_from_slice(&self.replica.count().to_be_bytes());
        bytes[Self::NONCE_RANGE].copy_from_slice(&self.nonce);
        bytes
    }

    /// Reads the header off the front of an object.
    ///
    /// Every check here is on plaintext bytes, so an object that is not a
    /// control object v1 is rejected without a key ever being used.
    pub fn parse(object: &[u8]) -> Result<Self> {
        let bytes = object
            .get(..Self::LEN)
            .ok_or(Error::ControlHeaderTooShort {
                actual: object.len(),
            })?;
        let magic: [u8; Self::MAGIC_LEN] = bytes[..Self::MAGIC_LEN]
            .try_into()
            .expect("the slice is MAGIC_LEN long");
        if magic != Self::MAGIC {
            return Err(Error::UnknownControlMagic { actual: magic });
        }
        let version = bytes[Self::VERSION_OFFSET];
        if version != Self::VERSION {
            return Err(Error::UnsupportedControlVersion { actual: version });
        }
        let kind = kind_from_byte(bytes[Self::KIND_OFFSET])?;
        if bytes[Self::RESERVED_OFFSET] != 0 {
            return Err(Error::ReservedNotZero);
        }
        let generation = Generation::new(u64::from_be_bytes(
            bytes[Self::GENERATION_RANGE]
                .try_into()
                .expect("the slice is 8 bytes long"),
        ));
        let replica = ReplicaPosition::new(
            u16::from_be_bytes(
                bytes[Self::REPLICA_INDEX_RANGE]
                    .try_into()
                    .expect("the slice is 2 bytes long"),
            ),
            u16::from_be_bytes(
                bytes[Self::REPLICA_COUNT_RANGE]
                    .try_into()
                    .expect("the slice is 2 bytes long"),
            ),
        )?;
        let nonce: [u8; nonce::LEN] = bytes[Self::NONCE_RANGE]
            .try_into()
            .expect("the slice is nonce::LEN long");
        Ok(Self::new(kind, generation, replica, nonce))
    }
}

fn kind_byte(kind: ControlObjectKind) -> u8 {
    match kind {
        ControlObjectKind::Journal => KIND_JOURNAL,
        ControlObjectKind::Keyring => KIND_KEYRING,
        ControlObjectKind::IndexSnapshot => KIND_INDEX_SNAPSHOT,
    }
}

fn kind_from_byte(byte: u8) -> Result<ControlObjectKind> {
    match byte {
        KIND_JOURNAL => Ok(ControlObjectKind::Journal),
        KIND_KEYRING => Ok(ControlObjectKind::Keyring),
        KIND_INDEX_SNAPSHOT => Ok(ControlObjectKind::IndexSnapshot),
        actual => Err(Error::UnknownControlObjectKind { actual }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::testing::ALL_KINDS;

    fn sample() -> ControlHeader {
        ControlHeader::new(
            ControlObjectKind::Keyring,
            Generation::new(7),
            ReplicaPosition::new(1, 3).expect("replica 1 of 3 is a valid position"),
            [0x2b; nonce::LEN],
        )
    }

    #[test]
    fn round_trips_through_bytes() {
        let header = sample();
        assert_eq!(ControlHeader::parse(&header.to_bytes()), Ok(header));
    }

    // FM-11: the header is magic "CFCTL", format version 0x01, the kind byte, a
    // reserved byte, the generation, the replica index and count, and the
    // nonce, at those exact offsets, with multi-byte integers big-endian.
    #[test]
    fn layout_matches_the_field_table() {
        let bytes = sample().to_bytes();
        assert_eq!(bytes.len(), 44);
        assert_eq!(&bytes[..5], b"CFCTL");
        assert_eq!(bytes[5], 0x01);
        assert_eq!(bytes[6], 0x02);
        assert_eq!(bytes[7], 0x00);
        assert_eq!(&bytes[8..16], &7u64.to_be_bytes());
        assert_eq!(&bytes[16..18], &1u16.to_be_bytes());
        assert_eq!(&bytes[18..20], &3u16.to_be_bytes());
        assert_eq!(&bytes[20..44], &[0x2b; 24]);
    }

    // FM-11: the kind byte is 0x01 for a Journal record, 0x02 for a Keyring
    // replica, and 0x03 for an Index Snapshot.
    #[test]
    fn kind_bytes_match_the_rule() {
        assert_eq!(kind_byte(ControlObjectKind::Journal), 0x01);
        assert_eq!(kind_byte(ControlObjectKind::Keyring), 0x02);
        assert_eq!(kind_byte(ControlObjectKind::IndexSnapshot), 0x03);
        for kind in ALL_KINDS {
            assert_eq!(kind_from_byte(kind_byte(kind)), Ok(kind));
        }
    }

    // FM-11: a future control-object kind takes a new kind byte, so a byte this
    // build does not know names no kind it can open.
    #[test]
    fn an_unknown_kind_byte_is_rejected() {
        let mut bytes = sample().to_bytes();
        bytes[6] = 0x04;
        assert_eq!(
            ControlHeader::parse(&bytes),
            Err(Error::UnknownControlObjectKind { actual: 0x04 })
        );
    }

    // FM-11: reserved bytes must be zero, as they must in a Container (FM-2).
    #[test]
    fn the_reserved_byte_must_be_zero() {
        let mut bytes = sample().to_bytes();
        bytes[7] = 0x01;
        assert_eq!(ControlHeader::parse(&bytes), Err(Error::ReservedNotZero));
    }

    #[test]
    fn short_input_is_rejected() {
        assert_eq!(
            ControlHeader::parse(b"CFCTL"),
            Err(Error::ControlHeaderTooShort { actual: 5 })
        );
    }

    // FM-11, FM-12: a replica index outside the count the header declares is
    // not a position any replica can hold.
    #[test]
    fn an_inconsistent_replica_position_is_rejected() {
        let mut bytes = sample().to_bytes();
        bytes[16..18].copy_from_slice(&3u16.to_be_bytes());
        assert_eq!(
            ControlHeader::parse(&bytes),
            Err(Error::Model(coffret_model::Error::InvalidReplicaPosition {
                index: 3,
                count: 3
            }))
        );
    }
}
