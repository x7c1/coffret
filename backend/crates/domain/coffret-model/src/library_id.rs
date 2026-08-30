use crate::error::{Error, Result};
use crate::lowercase_hex;
use std::fmt;

/// The 64-bit identifier one Library is named on Storage by (spec: FM-18).
///
/// It is drawn from a CSPRNG when the Library is created and takes no input
/// from the Master Key: rotating the key must not rename the folder every other
/// device is configured against, and a name Storage can read must not be a
/// function of key material. Generation needs an entropy source and therefore
/// lives in `coffret-format`; this type only holds and formats the value.
///
/// 64 bits is what the name has to carry, and no more: the ID separates the
/// Libraries a person keeps at one Storage location from each other, and a
/// device recovering with only a Recovery Code enumerates those few names. It
/// is not a secret and not an authenticator — which Library a name belongs to
/// is settled by opening its Keyring under a Master Key, never by the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LibraryId([u8; Self::BYTE_LEN]);

impl LibraryId {
    /// Length of a Library ID in bytes.
    pub const BYTE_LEN: usize = 8;

    /// Length of a Library ID in hex characters.
    pub const HEX_LEN: usize = Self::BYTE_LEN * 2;

    /// What the name of every Library's app folder starts with.
    ///
    /// The prefix is what makes the folder discoverable: a recovering device
    /// lists it at the Storage location and opens each candidate's Keyring
    /// (spec: FM-18).
    pub const APP_FOLDER_PREFIX: &'static str = "coffret-";

    /// Takes 8 raw bytes.
    pub const fn from_bytes(bytes: [u8; Self::BYTE_LEN]) -> Self {
        Self(bytes)
    }

    /// The raw 8 bytes.
    pub const fn as_bytes(&self) -> &[u8; Self::BYTE_LEN] {
        &self.0
    }

    /// Parses the 16-lowercase-hex-character spelling.
    pub fn from_hex(hex: &str) -> Result<Self> {
        lowercase_hex::decode(hex).map(Self)
    }

    /// The 16-lowercase-hex-character spelling.
    pub fn to_hex(&self) -> String {
        lowercase_hex::encode(&self.0)
    }

    /// The name of the folder this Library's objects live in: `coffret-` and
    /// the ID (spec: FM-18).
    pub fn app_folder_name(&self) -> String {
        let mut name = String::with_capacity(Self::APP_FOLDER_PREFIX.len() + Self::HEX_LEN);
        name.push_str(Self::APP_FOLDER_PREFIX);
        name.push_str(&self.to_hex());
        name
    }

    /// The same folder on a Storage that keys objects by name: the app folder's
    /// name as a key prefix under `base` (spec: FM-18).
    ///
    /// `base` is where the user put the Library — the empty string for the root
    /// of a bucket, otherwise a prefix ending in `/`. Anything else is refused
    /// rather than repaired: a base of `photos` would silently place the
    /// Library at `photoscoffret-…/`, which is a different location from the
    /// `photos/` the caller meant, and one nothing would ever find it under.
    pub fn app_prefix(&self, base: &str) -> Result<String> {
        if !base.is_empty() && !base.ends_with('/') {
            return Err(Error::MalformedPrefixBase {
                base: base.to_owned(),
            });
        }
        Ok(format!("{base}{}/", self.app_folder_name()))
    }
}

impl fmt::Display for LibraryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LibraryId {
        LibraryId::from_bytes([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77])
    }

    // FM-18: a Library's app folder is `coffret-` and the ID as 16 lowercase
    // hex characters.
    #[test]
    fn an_app_folder_is_named_after_the_library() {
        assert_eq!(sample().app_folder_name(), "coffret-0011223344556677");
    }

    // The lowercase spelling of every digit, and the leading zero a byte under
    // 0x10 keeps, are what make one ID one name.
    #[test]
    fn every_nibble_is_spelled_in_lowercase_hex() {
        let id = LibraryId::from_bytes([0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);

        assert_eq!(id.to_hex(), "0123456789abcdef");
        assert_eq!(id.to_string(), id.to_hex());
    }

    #[test]
    fn hex_round_trips() {
        let id = sample();
        assert_eq!(
            LibraryId::from_hex(&id.to_hex()).expect("an ID's own hex parses back"),
            id
        );
        assert_eq!(id.as_bytes().len(), LibraryId::BYTE_LEN);
    }

    #[test]
    fn from_hex_rejects_uppercase() {
        let result = LibraryId::from_hex("0123456789ABCDEF");
        assert!(
            matches!(result, Err(Error::InvalidHexDigit { found: 'A' })),
            "expected 'A' to be rejected as a hex digit, got {result:?}"
        );
    }

    #[test]
    fn from_hex_rejects_a_length_other_than_sixteen() {
        for (hex, characters) in [("001122334455667", 15), ("00112233445566778", 17)] {
            let result = LibraryId::from_hex(hex);
            assert!(
                matches!(
                    result,
                    Err(Error::InvalidHexLength {
                        expected: 16,
                        actual
                    }) if actual == characters
                ),
                "expected 16 hex characters and found {characters}, got {result:?}"
            );
        }
    }

    #[test]
    fn from_hex_rejects_a_character_that_is_not_hex() {
        let result = LibraryId::from_hex("001122334455667z");
        assert!(
            matches!(result, Err(Error::InvalidHexDigit { found: 'z' })),
            "expected 'z' to be rejected as a hex digit, got {result:?}"
        );
    }

    // FM-18: on a Storage that keys objects by name, the app folder is the key
    // prefix the Library's objects are written under.
    #[test]
    fn an_app_prefix_is_the_folder_name_under_the_base() {
        let id = sample();

        assert_eq!(
            id.app_prefix("").expect("an empty base is the bucket root"),
            "coffret-0011223344556677/"
        );
        assert_eq!(
            id.app_prefix("photos/")
                .expect("a base ending in a separator is a prefix"),
            "photos/coffret-0011223344556677/"
        );
    }

    #[test]
    fn an_app_prefix_refuses_a_base_that_does_not_end_in_a_separator() {
        let result = sample().app_prefix("photos");
        assert!(
            matches!(result, Err(Error::MalformedPrefixBase { ref base }) if base == "photos"),
            "expected a base without a trailing separator to be refused, got {result:?}"
        );
    }
}
