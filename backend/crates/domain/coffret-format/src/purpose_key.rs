use std::fmt;

use coffret_model::MasterKey;
use hkdf::Hkdf;
use sha2::Sha256;

use crate::aead::KEY_LEN;
use crate::error::{Error, Result};
use crate::purpose::Purpose;

/// A 256-bit key derived from the Master Key for exactly one purpose.
///
/// Derivation is HKDF-SHA-256 with the Master Key as input keying material, a
/// zero-length salt, the purpose's info string, and a 32-byte output. The
/// Master Key itself never encrypts anything, so a purpose that leaks costs the
/// Library that purpose and nothing else.
///
/// A key carries the purpose it was derived for, and every operation that takes
/// one checks that purpose before using it — separate keys only separate
/// anything if nothing crosses them over.
///
/// `Debug` is redacted, and the type implements neither `Display` nor
/// `PartialEq`, for the same reasons [`coffret_model::ContainerKey`] does.
#[derive(Clone)]
pub struct PurposeKey {
    purpose: Purpose,
    bytes: [u8; KEY_LEN],
}

impl PurposeKey {
    /// Length of a purpose key in bytes.
    pub const BYTE_LEN: usize = KEY_LEN;

    /// Derives the key for one purpose from the Master Key.
    pub fn derive(master_key: &MasterKey, purpose: Purpose) -> Self {
        let hkdf = Hkdf::<Sha256>::new(Some(&[]), master_key.as_bytes());
        let mut bytes = [0u8; KEY_LEN];
        hkdf.expand(purpose.info().as_bytes(), &mut bytes)
            .expect("32 bytes is far below HKDF-SHA-256's output limit");
        Self { purpose, bytes }
    }

    /// What this key is allowed to encrypt.
    pub const fn purpose(&self) -> Purpose {
        self.purpose
    }

    /// The raw key bytes, once the caller's purpose matches this key's.
    pub(crate) fn require(&self, expected: Purpose) -> Result<&[u8; KEY_LEN]> {
        if self.purpose != expected {
            return Err(Error::WrongPurposeKey {
                expected,
                actual: self.purpose,
            });
        }
        Ok(&self.bytes)
    }
}

impl fmt::Debug for PurposeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PurposeKey({}, <redacted>)", self.purpose)
    }
}

#[cfg(test)]
mod tests {
    use coffret_model::ContainerKey;

    use super::*;
    use crate::aead::Cipher;
    use crate::purpose::ALL;

    fn master_key() -> MasterKey {
        // A Master Key whose every byte differs, so a derivation that dropped
        // or reordered input bytes would not land on the same output.
        let mut bytes = [0u8; MasterKey::BYTE_LEN];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = index as u8;
        }
        MasterKey::from_bytes(bytes)
    }

    fn derived(purpose: Purpose) -> [u8; KEY_LEN] {
        *PurposeKey::derive(&master_key(), purpose)
            .require(purpose)
            .expect("the purpose matches")
    }

    // KD-3, KD-4: purpose keys are HKDF-SHA-256 over the Master Key with a
    // zero-length salt, the purpose's info string, and a 32-byte output. These
    // vectors were computed independently from RFC 5869 for the Master Key
    // above; any drift in salt, info string, or output length breaks them, and
    // with them every object already written.
    #[test]
    fn derivation_matches_the_pinned_vectors() {
        assert_eq!(
            derived(Purpose::ContainerWrap),
            [
                0xef, 0x89, 0x47, 0xe4, 0xd7, 0x83, 0x1b, 0xe5, 0xc1, 0x89, 0x44, 0x89, 0xe2, 0xfa,
                0x1e, 0x6a, 0xd0, 0xf3, 0x5e, 0x84, 0xbe, 0x80, 0x55, 0x2c, 0x81, 0x0b, 0x44, 0xe4,
                0x05, 0x8b, 0xe5, 0x1b,
            ]
        );
        assert_eq!(
            derived(Purpose::ControlJournal),
            [
                0xb3, 0xef, 0x1d, 0x17, 0x4a, 0x07, 0xe6, 0xeb, 0xc7, 0x30, 0x90, 0xad, 0x90, 0x8a,
                0x36, 0x18, 0xbe, 0x34, 0x84, 0x0c, 0x45, 0xf8, 0x85, 0x28, 0x31, 0x58, 0x69, 0x4a,
                0x95, 0x49, 0x60, 0x40,
            ]
        );
        assert_eq!(
            derived(Purpose::ControlKeyring),
            [
                0x92, 0x16, 0x29, 0xb1, 0x9a, 0x4d, 0xfc, 0xa1, 0x69, 0x32, 0x01, 0xfe, 0x25, 0xc6,
                0xd5, 0xaa, 0x90, 0x15, 0x0f, 0xae, 0x50, 0x35, 0x92, 0xae, 0xe0, 0x8f, 0x4d, 0x1d,
                0x70, 0xdc, 0x6f, 0x1d,
            ]
        );
        assert_eq!(
            derived(Purpose::ControlIndexSnapshot),
            [
                0x10, 0xd7, 0x0a, 0xdb, 0xee, 0x11, 0xad, 0x0f, 0xb7, 0x19, 0x09, 0x42, 0xc7, 0x92,
                0x3b, 0xe2, 0xaa, 0xe9, 0xf4, 0xf5, 0x0d, 0xfd, 0x29, 0xee, 0xf5, 0x69, 0xdb, 0xe4,
                0x8b, 0xd8, 0xe2, 0x5c,
            ]
        );
        assert_eq!(
            derived(Purpose::TokenCache),
            [
                0xde, 0x5b, 0x77, 0xda, 0x95, 0x08, 0x82, 0x1a, 0x4f, 0x96, 0x51, 0xad, 0xe2, 0x24,
                0x93, 0xc3, 0x99, 0xb4, 0xc0, 0xa6, 0x87, 0xff, 0x27, 0x54, 0x25, 0xd2, 0x28, 0xd8,
                0x1c, 0x39, 0x1f, 0x75,
            ]
        );
    }

    // KD-3: the Master Key is never used directly as an AEAD key — every
    // purpose key differs from it and from every other purpose key.
    #[test]
    fn no_purpose_key_repeats_another_or_the_master_key() {
        let keys: Vec<[u8; KEY_LEN]> = ALL.iter().map(|purpose| derived(*purpose)).collect();
        for (index, key) in keys.iter().enumerate() {
            assert_ne!(key, master_key().as_bytes());
            for other in &keys[index + 1..] {
                assert_ne!(key, other);
            }
        }
    }

    // KD-3: derivation is deterministic — the same Master Key and purpose
    // always yield the same key, which is what lets any device open what
    // another wrote.
    #[test]
    fn derivation_is_deterministic() {
        for purpose in ALL {
            assert_eq!(derived(purpose), derived(purpose));
        }
    }

    // KD-4: a payload sealed under one purpose key opens under no other purpose
    // key, and not under a Container Key either — the separation is
    // cryptographic, not just a label this crate checks.
    #[test]
    fn a_payload_sealed_under_one_purpose_key_opens_under_no_other() {
        let sealed_under = Purpose::ControlJournal;
        let nonce = crate::nonce::meta();
        let mut sealed = Vec::new();
        Cipher::new(&derived(sealed_under))
            .seal(&nonce, b"ad", &mut b"payload".to_vec(), &mut sealed)
            .expect("sealing succeeds");

        let mut wrong: Vec<[u8; KEY_LEN]> = ALL
            .iter()
            .filter(|purpose| **purpose != sealed_under)
            .map(|purpose| derived(*purpose))
            .collect();
        // A Container Key is drawn independently of the Master Key (KD-2), so it
        // is no more able to open this than a wrong purpose key is.
        wrong.push(*ContainerKey::from_bytes([0x11; ContainerKey::BYTE_LEN]).as_bytes());

        for key in wrong {
            let result = Cipher::new(&key).open(&nonce, b"ad", &sealed);
            assert!(
                matches!(result, Err(Error::AuthenticationFailed)),
                "no key but the one it was sealed under should open it, got {result:?}"
            );
        }
        assert_eq!(
            Cipher::new(&derived(sealed_under))
                .open(&nonce, b"ad", &sealed)
                .expect("the key it was sealed under opens it"),
            b"payload".to_vec()
        );
    }

    // KD-4: a key derived for one purpose is not accepted for another.
    #[test]
    fn a_key_refuses_a_purpose_it_was_not_derived_for() {
        let key = PurposeKey::derive(&master_key(), Purpose::ControlJournal);
        let result = key.require(Purpose::ControlKeyring);
        assert!(
            matches!(
                result,
                Err(Error::WrongPurposeKey {
                    expected: Purpose::ControlKeyring,
                    actual: Purpose::ControlJournal,
                })
            ),
            "expected a Journal key to be refused for the Keyring, got {result:?}"
        );
        assert!(key.require(Purpose::ControlJournal).is_ok());
    }

    #[test]
    fn debug_does_not_leak_key_material() {
        let key = PurposeKey::derive(&master_key(), Purpose::ContainerWrap);
        assert_eq!(
            format!("{key:?}"),
            "PurposeKey(coffret/v1/container-wrap, <redacted>)"
        );
    }
}
