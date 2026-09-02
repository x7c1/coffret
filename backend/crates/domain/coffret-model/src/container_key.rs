use std::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// The 256-bit key that encrypts exactly one Container.
///
/// `Debug` is redacted so key material cannot reach a log line through a
/// derived formatter, and the type deliberately implements neither `Display`
/// nor `PartialEq` — an equality operator would have to be constant-time, and
/// nothing in the domain needs to compare two keys.
///
/// It is neither `Copy` nor `Clone`, and it overwrites its bytes when it is
/// dropped, for the reason [`MasterKey`] does and under the same list
/// (spec: DK-7).
///
/// [`MasterKey`]: crate::MasterKey
pub struct ContainerKey([u8; Self::BYTE_LEN]);

impl ContainerKey {
    /// Length of a Container Key in bytes.
    pub const BYTE_LEN: usize = 32;

    /// Takes 32 raw bytes.
    ///
    /// Whoever produced the bytes wipes them: the generator hands over the only
    /// copy, and unwrapping a Key Envelope reads them out of a `Zeroizing`
    /// buffer.
    pub const fn from_bytes(bytes: [u8; Self::BYTE_LEN]) -> Self {
        Self(bytes)
    }

    /// The raw 32 bytes.
    pub const fn as_bytes(&self) -> &[u8; Self::BYTE_LEN] {
        &self.0
    }
}

impl fmt::Debug for ContainerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ContainerKey(<redacted>)")
    }
}

impl Drop for ContainerKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl ZeroizeOnDrop for ContainerKey {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_leak_key_material() {
        let key = ContainerKey::from_bytes([0xab; ContainerKey::BYTE_LEN]);
        assert_eq!(format!("{key:?}"), "ContainerKey(<redacted>)");
    }

    // DK-7, checked the way `MasterKey`'s is: on the operation the drop runs.
    #[test]
    fn the_drop_time_wipe_overwrites_the_key() {
        let mut key = ContainerKey::from_bytes([0xab; ContainerKey::BYTE_LEN]);
        key.0.zeroize();
        assert_eq!(key.as_bytes(), &[0u8; ContainerKey::BYTE_LEN]);
    }
}
