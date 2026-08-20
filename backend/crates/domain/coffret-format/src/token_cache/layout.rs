use super::{offset, HEADER_LEN, MAGIC, MAGIC_LEN, VERSION};
use crate::aead::TAG_LEN;
use crate::error::{Error, Result};
use crate::nonce;

/// What the plaintext part of one sealed cache says about the rest of it.
pub(super) struct Layout {
    pub(super) nonce: [u8; nonce::LEN],
    /// The `ciphertext ‖ tag` part of the file; where it starts is also how
    /// long the associated data is.
    pub(super) message: std::ops::Range<usize>,
}

impl Layout {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self> {
        let too_short = || Error::TokenCacheTooShort {
            actual: bytes.len(),
        };
        let header = bytes.get(..HEADER_LEN).ok_or_else(too_short)?;

        // Shape first, key afterwards: bytes that are not this form at all are
        // told apart from bytes that are but fail to open, and neither answer
        // needs the Master Key to reach it.
        let magic: [u8; MAGIC_LEN] = header[..MAGIC_LEN]
            .try_into()
            .expect("the slice is MAGIC_LEN long");
        if magic != MAGIC {
            return Err(Error::UnknownTokenCacheMagic { actual: magic });
        }
        let version = header[offset::VERSION];
        if version != VERSION {
            return Err(Error::UnsupportedTokenCacheVersion { actual: version });
        }
        if header[offset::RESERVED] != 0 {
            return Err(Error::ReservedNotZero);
        }
        let nonce: [u8; nonce::LEN] = header[offset::NONCE..]
            .try_into()
            .expect("the slice is nonce::LEN long");

        // A cache holding nothing is still a tag long; anything shorter is a
        // file that was cut off rather than one that was written.
        if bytes.len() < HEADER_LEN + TAG_LEN {
            return Err(too_short());
        }
        Ok(Self {
            nonce,
            message: HEADER_LEN..bytes.len(),
        })
    }
}
