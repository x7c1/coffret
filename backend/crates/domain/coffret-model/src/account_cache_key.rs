use std::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// The 256-bit key one Storage account's sealed token cache is kept under on a
/// device (spec: KD-12).
///
/// Drawn at random when a device first keeps a grant for the account, and
/// derived from no Master Key: that is what lets Libraries holding different
/// Master Keys open one account's cache, each through an envelope of its own
/// that wraps this key under that Library's own purpose key.
///
/// `Debug` is redacted, and the type implements neither `Display` nor
/// `PartialEq`, for the reasons [`MasterKey`] does. It is neither `Copy` nor
/// `Clone`, and it overwrites its bytes when it is dropped: it opens a refresh
/// token that is a bearer credential for every object this application created
/// in the account, so it is on the secret-bearing inventory with the rest
/// (spec: DK-7).
///
/// [`MasterKey`]: crate::MasterKey
pub struct AccountCacheKey([u8; Self::BYTE_LEN]);

impl AccountCacheKey {
    /// Length of an account-cache key in bytes.
    pub const BYTE_LEN: usize = 32;

    /// Takes 32 raw bytes.
    ///
    /// Whoever produced the bytes wipes them: the generator hands over the only
    /// copy, and opening an envelope reads them out of a `Zeroizing` buffer.
    pub const fn from_bytes(bytes: [u8; Self::BYTE_LEN]) -> Self {
        Self(bytes)
    }

    /// The raw 32 bytes.
    pub const fn as_bytes(&self) -> &[u8; Self::BYTE_LEN] {
        &self.0
    }
}

impl fmt::Debug for AccountCacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AccountCacheKey(<redacted>)")
    }
}

impl Drop for AccountCacheKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl ZeroizeOnDrop for AccountCacheKey {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_leak_key_material() {
        let key = AccountCacheKey::from_bytes([0xab; AccountCacheKey::BYTE_LEN]);
        assert_eq!(format!("{key:?}"), "AccountCacheKey(<redacted>)");
    }

    // DK-7, checked the way `MasterKey`'s is: on the operation the drop runs.
    #[test]
    fn the_drop_time_wipe_overwrites_the_key() {
        let mut key = AccountCacheKey::from_bytes([0xab; AccountCacheKey::BYTE_LEN]);
        key.0.zeroize();
        assert_eq!(key.as_bytes(), &[0u8; AccountCacheKey::BYTE_LEN]);
    }
}
