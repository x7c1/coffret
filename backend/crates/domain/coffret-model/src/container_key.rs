use std::fmt;

/// The 256-bit key that encrypts exactly one Container.
///
/// `Debug` is redacted so key material cannot reach a log line through a
/// derived formatter, and the type deliberately implements neither `Display`
/// nor `PartialEq` — an equality operator would have to be constant-time, and
/// nothing in the domain needs to compare two keys.
///
/// Zeroization on drop is not implemented: every crate-level zeroizing helper
/// is a third-party dependency, and this crate takes none. The type is kept
/// non-`Copy` so a key at least has a single owner to reason about.
#[derive(Clone)]
pub struct ContainerKey([u8; Self::BYTE_LEN]);

impl ContainerKey {
    /// Length of a Container Key in bytes.
    pub const BYTE_LEN: usize = 32;

    /// Wraps 32 raw bytes.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_leak_key_material() {
        let key = ContainerKey::from_bytes([0xab; ContainerKey::BYTE_LEN]);
        assert_eq!(format!("{key:?}"), "ContainerKey(<redacted>)");
    }
}
