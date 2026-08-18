//! The one AEAD construction format v1 uses.
//!
//! Every AEAD message in a Container — the meta section and each chunk — is
//! XChaCha20-Poly1305 with a 256-bit key and a 24-byte nonce, laid down as
//! `ciphertext ‖ tag(16)`. A message that fails authentication is rejected
//! whole: this module never hands back plaintext it could not authenticate.

use chacha20poly1305::aead::inout::InOutBuf;
use chacha20poly1305::aead::{AeadInOut, Tag};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use coffret_model::ContainerKey;

use crate::error::{Error, Result};
use crate::nonce;

/// Length of a Poly1305 authentication tag in bytes.
pub(crate) const TAG_LEN: usize = 16;

pub(crate) struct Cipher(XChaCha20Poly1305);

impl Cipher {
    pub(crate) fn new(key: &ContainerKey) -> Self {
        Self(XChaCha20Poly1305::new(key.as_bytes().into()))
    }

    /// Encrypts `plaintext` in place and appends `ciphertext ‖ tag` to `out`.
    pub(crate) fn seal(
        &self,
        nonce: &[u8; nonce::LEN],
        associated_data: &[u8],
        plaintext: &mut [u8],
        out: &mut Vec<u8>,
    ) -> Result<()> {
        let tag = self
            .0
            .encrypt_inout_detached(
                &XNonce::from(*nonce),
                associated_data,
                InOutBuf::from(&mut *plaintext),
            )
            .map_err(|_| Error::AuthenticationFailed)?;

        out.extend_from_slice(plaintext);
        out.extend_from_slice(&tag);
        Ok(())
    }

    /// Authenticates `message` (`ciphertext ‖ tag`) and returns its plaintext.
    ///
    /// The plaintext is built in a buffer this call owns and is returned only
    /// once the tag verifies, so a caller can never observe bytes from an
    /// unauthenticated message.
    pub(crate) fn open(
        &self,
        nonce: &[u8; nonce::LEN],
        associated_data: &[u8],
        message: &[u8],
    ) -> Result<Vec<u8>> {
        let split = message.len().checked_sub(TAG_LEN).ok_or(Error::Truncated)?;
        let (ciphertext, tag) = message.split_at(split);
        let tag = Tag::<XChaCha20Poly1305>::try_from(tag).expect("the slice is TAG_LEN long");

        let mut buffer = ciphertext.to_vec();
        self.0
            .decrypt_inout_detached(
                &XNonce::from(*nonce),
                associated_data,
                InOutBuf::from(&mut buffer[..]),
                &tag,
            )
            .map_err(|_| Error::AuthenticationFailed)?;

        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cipher() -> Cipher {
        Cipher::new(&ContainerKey::from_bytes([0x2a; ContainerKey::BYTE_LEN]))
    }

    fn seal(plaintext: &[u8], associated_data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        cipher()
            .seal(
                &nonce::meta(),
                associated_data,
                &mut plaintext.to_vec(),
                &mut out,
            )
            .expect("sealing cannot fail");
        out
    }

    // FM-1: every AEAD operation is XChaCha20-Poly1305 with a 256-bit key and a
    // 24-byte nonce, laid down as ciphertext followed by a 16-byte tag.
    #[test]
    fn sealed_message_is_ciphertext_plus_tag() {
        let sealed = seal(b"hello", b"ad");
        assert_eq!(sealed.len(), b"hello".len() + TAG_LEN);
        assert_ne!(&sealed[..5], b"hello");
    }

    #[test]
    fn round_trips() {
        let sealed = seal(b"hello", b"ad");
        let opened = cipher()
            .open(&nonce::meta(), b"ad", &sealed)
            .expect("the message is intact");
        assert_eq!(opened, b"hello");
    }

    // FM-1: a message that fails authentication is rejected whole, and no
    // plaintext from it is released downstream.
    #[test]
    fn tampered_message_is_rejected() {
        let mut sealed = seal(b"hello", b"ad");
        sealed[0] ^= 0x01;
        assert_eq!(
            cipher().open(&nonce::meta(), b"ad", &sealed),
            Err(Error::AuthenticationFailed)
        );
    }

    #[test]
    fn wrong_associated_data_is_rejected() {
        let sealed = seal(b"hello", b"ad");
        assert_eq!(
            cipher().open(&nonce::meta(), b"other", &sealed),
            Err(Error::AuthenticationFailed)
        );
    }

    #[test]
    fn wrong_nonce_is_rejected() {
        let sealed = seal(b"hello", b"ad");
        assert_eq!(
            cipher().open(&nonce::chunk(0, true), b"ad", &sealed),
            Err(Error::AuthenticationFailed)
        );
    }

    #[test]
    fn message_shorter_than_a_tag_is_rejected() {
        assert_eq!(
            cipher().open(&nonce::meta(), b"ad", &[0u8; TAG_LEN - 1]),
            Err(Error::Truncated)
        );
    }
}
