use super::{offset, Argon2Params, StoredMasterKey, PLAINTEXT_LEN};
use crate::aead::TAG_LEN;
use crate::error::{Error, Result};
use crate::nonce;

/// What the plaintext part of one stored form says about the rest of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Layout {
    pub(super) params: Argon2Params,
    pub(super) salt: std::ops::Range<usize>,
    pub(super) nonce: std::ops::Range<usize>,
    pub(super) message: std::ops::Range<usize>,
}

impl Layout {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self> {
        let fixed = bytes
            .get(..offset::SALT)
            .ok_or(Error::StoredMasterKeyLengthMismatch)?;
        let magic: [u8; StoredMasterKey::MAGIC_LEN] = fixed[..StoredMasterKey::MAGIC_LEN]
            .try_into()
            .expect("the slice is MAGIC_LEN long");
        if magic != StoredMasterKey::MAGIC {
            return Err(Error::UnknownStoredMasterKeyMagic { actual: magic });
        }
        let version = fixed[offset::VERSION];
        if version != StoredMasterKey::VERSION {
            return Err(Error::UnsupportedStoredMasterKeyVersion { actual: version });
        }
        if fixed[offset::RESERVED] != 0 {
            return Err(Error::ReservedNotZero);
        }
        let params = Argon2Params::new(
            read_u32(fixed, offset::MEMORY_KIB),
            read_u32(fixed, offset::ITERATIONS),
            read_u32(fixed, offset::PARALLELISM),
        );

        let salt = offset::SALT..offset::SALT + usize::from(fixed[offset::SALT_LEN]);
        let nonce = salt.end..salt.end + nonce::LEN;
        let message = nonce.end..nonce.end + PLAINTEXT_LEN + TAG_LEN;
        // The encrypted plaintext is a key and an epoch and nothing else, so the
        // whole form is exactly this long — no shorter, and with nothing
        // appended.
        if bytes.len() != message.end {
            return Err(Error::StoredMasterKeyLengthMismatch);
        }
        Ok(Self {
            params,
            salt,
            nonce,
            message,
        })
    }
}

fn read_u32(bytes: &[u8], range: std::ops::Range<usize>) -> u32 {
    u32::from_be_bytes(bytes[range].try_into().expect("the slice is 4 bytes long"))
}
