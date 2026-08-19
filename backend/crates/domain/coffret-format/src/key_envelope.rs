//! Key Envelopes: a Container Key wrapped under the container-wrap purpose key.
//!
//! An envelope is `nonce(24) ‖ ciphertext(32) ‖ tag(16)` — 72 bytes — with the
//! 16-byte Container ID as associated data, so an envelope presented for a
//! different Container fails to unwrap and envelopes cannot be swapped between
//! Containers. Envelopes live in the Keyring and never in a Container itself,
//! which is what lets a Master Key rotation rewrite every envelope while
//! leaving Containers byte-for-byte unchanged.

use coffret_model::{ContainerId, ContainerKey, KeyEnvelope};

use crate::aead::Cipher;
use crate::error::{Error, Result};
use crate::nonce;
use crate::purpose::Purpose;
use crate::purpose_key::PurposeKey;

/// Where the ciphertext starts inside an envelope, after the nonce.
const CIPHERTEXT_OFFSET: usize = nonce::LEN;

/// Wraps a Container Key into the envelope the Keyring stores for it.
pub fn wrap_container_key(
    key: &PurposeKey,
    container_id: &ContainerId,
    container_key: &ContainerKey,
) -> Result<KeyEnvelope> {
    let cipher = Cipher::new(key.require(Purpose::ContainerWrap)?);
    let nonce = nonce::random()?;

    let mut envelope = Vec::with_capacity(KeyEnvelope::BYTE_LEN);
    envelope.extend_from_slice(&nonce);
    cipher.seal(
        &nonce,
        container_id.as_bytes(),
        &mut container_key.as_bytes().to_vec(),
        &mut envelope,
    )?;
    KeyEnvelope::from_slice(&envelope).map_err(Error::from)
}

/// Opens the envelope a Keyring holds for one Container.
///
/// The Container ID goes in as associated data rather than being read out of
/// the envelope, so an envelope that belongs to another Container fails
/// authentication instead of yielding the wrong key.
pub fn unwrap_container_key(
    key: &PurposeKey,
    container_id: &ContainerId,
    envelope: &KeyEnvelope,
) -> Result<ContainerKey> {
    let cipher = Cipher::new(key.require(Purpose::ContainerWrap)?);
    let bytes = envelope.as_bytes();
    let nonce: [u8; nonce::LEN] = bytes[..CIPHERTEXT_OFFSET]
        .try_into()
        .expect("the slice is nonce::LEN long");

    let plaintext = cipher.open(&nonce, container_id.as_bytes(), &bytes[CIPHERTEXT_OFFSET..])?;
    let plaintext: [u8; ContainerKey::BYTE_LEN] = plaintext
        .try_into()
        .expect("an envelope's fixed length leaves exactly a Container Key");
    Ok(ContainerKey::from_bytes(plaintext))
}

#[cfg(test)]
mod tests {
    use coffret_model::MasterKey;

    use super::*;
    use crate::aead::TAG_LEN;
    use crate::container_key::generate_container_key;
    use crate::purpose::ALL;

    fn container_wrap_key() -> PurposeKey {
        PurposeKey::derive(
            &MasterKey::from_bytes([0x5e; MasterKey::BYTE_LEN]),
            Purpose::ContainerWrap,
        )
    }

    fn container_id(byte: u8) -> ContainerId {
        ContainerId::from_bytes([byte; ContainerId::BYTE_LEN])
    }

    // FM-14: a Key Envelope is nonce(24) ‖ ciphertext(32) ‖ tag(16) — 72 bytes.
    #[test]
    fn an_envelope_is_seventy_two_bytes() {
        let envelope = wrap_container_key(
            &container_wrap_key(),
            &container_id(1),
            &ContainerKey::from_bytes([0x11; ContainerKey::BYTE_LEN]),
        )
        .expect("wrapping succeeds");

        assert_eq!(envelope.as_bytes().len(), 72);
        assert_eq!(
            envelope.as_bytes().len(),
            nonce::LEN + ContainerKey::BYTE_LEN + TAG_LEN
        );
    }

    // FM-14: the envelope carries the Container Key, and unwrapping it under
    // the same purpose key and Container ID returns exactly that key.
    #[test]
    fn wrapping_round_trips() {
        let container_key = generate_container_key().expect("the OS CSPRNG is available");
        let envelope = wrap_container_key(&container_wrap_key(), &container_id(2), &container_key)
            .expect("wrapping succeeds");

        let opened = unwrap_container_key(&container_wrap_key(), &container_id(2), &envelope)
            .expect("the envelope is intact");
        assert_eq!(opened.as_bytes(), container_key.as_bytes());
    }

    // FM-14: the nonce is fresh for every envelope, so wrapping the same key
    // for the same Container twice produces two different envelopes.
    #[test]
    fn every_envelope_gets_its_own_nonce() {
        let container_key = ContainerKey::from_bytes([0x33; ContainerKey::BYTE_LEN]);
        let first = wrap_container_key(&container_wrap_key(), &container_id(3), &container_key)
            .expect("wrapping succeeds");
        let second = wrap_container_key(&container_wrap_key(), &container_id(3), &container_key)
            .expect("wrapping succeeds");
        assert_ne!(first, second);
    }

    // FM-14: an envelope presented for a different Container fails to unwrap,
    // so envelopes cannot be swapped between Containers.
    #[test]
    fn an_envelope_does_not_open_for_another_container() {
        let container_key = ContainerKey::from_bytes([0x44; ContainerKey::BYTE_LEN]);
        let envelope = wrap_container_key(&container_wrap_key(), &container_id(4), &container_key)
            .expect("wrapping succeeds");

        assert_eq!(
            unwrap_container_key(&container_wrap_key(), &container_id(5), &envelope).err(),
            Some(Error::AuthenticationFailed)
        );
    }

    #[test]
    fn a_tampered_envelope_fails_to_unwrap() {
        let container_key = ContainerKey::from_bytes([0x55; ContainerKey::BYTE_LEN]);
        let envelope = wrap_container_key(&container_wrap_key(), &container_id(6), &container_key)
            .expect("wrapping succeeds");

        for index in 0..KeyEnvelope::BYTE_LEN {
            let mut bytes = *envelope.as_bytes();
            bytes[index] ^= 0x01;
            assert_eq!(
                unwrap_container_key(
                    &container_wrap_key(),
                    &container_id(6),
                    &KeyEnvelope::from_bytes(bytes)
                )
                .err(),
                Some(Error::AuthenticationFailed),
                "byte {index} was not authenticated"
            );
        }
    }

    // KD-4: the container-wrap key wraps Container Keys and nothing else does —
    // a key derived for another purpose is refused outright.
    #[test]
    fn only_the_container_wrap_key_may_wrap() {
        let master_key = MasterKey::from_bytes([0x5e; MasterKey::BYTE_LEN]);
        for purpose in ALL {
            if purpose == Purpose::ContainerWrap {
                continue;
            }
            let key = PurposeKey::derive(&master_key, purpose);
            assert_eq!(
                wrap_container_key(
                    &key,
                    &container_id(7),
                    &ContainerKey::from_bytes([0x66; ContainerKey::BYTE_LEN])
                ),
                Err(Error::WrongPurposeKey {
                    expected: Purpose::ContainerWrap,
                    actual: purpose,
                })
            );
        }
    }
}
