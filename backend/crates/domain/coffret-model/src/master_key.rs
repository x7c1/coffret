use std::fmt;

/// The 256-bit key every purpose key in a Library is derived from.
///
/// It is drawn from a CSPRNG and never from the Passphrase or any other
/// user-chosen input, so the strength of the ciphertext on Storage never
/// depends on passphrase quality. Each Master Key epoch draws its own.
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
pub struct MasterKey([u8; Self::BYTE_LEN]);

impl MasterKey {
    /// Length of a Master Key in bytes.
    pub const BYTE_LEN: usize = 32;

    /// Takes 32 raw bytes.
    pub const fn from_bytes(bytes: [u8; Self::BYTE_LEN]) -> Self {
        Self(bytes)
    }

    /// The raw 32 bytes.
    pub const fn as_bytes(&self) -> &[u8; Self::BYTE_LEN] {
        &self.0
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MasterKey(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_leak_key_material() {
        let key = MasterKey::from_bytes([0xab; MasterKey::BYTE_LEN]);
        assert_eq!(format!("{key:?}"), "MasterKey(<redacted>)");
    }
}
