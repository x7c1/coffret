use zeroize::Zeroizing;

use super::{token_cache_key, HEADER_LEN, MAGIC, VERSION};
use crate::aead::{Cipher, TAG_LEN};
use crate::error::Result;
use crate::nonce;
use crate::purpose_key::PurposeKey;

/// Seals a device's token cache under the token-cache purpose key.
///
/// The nonce is drawn fresh on every call: one key covers every cache this
/// device ever writes, so nothing but a random nonce keeps two writes from
/// sharing one. The bytes returned are the whole file — magic, version, nonce,
/// and the AEAD message — and the caller writes them as they are.
///
/// A key derived for any other purpose is refused rather than used (KD-4).
pub fn encode_token_cache(plaintext: &[u8], key: &PurposeKey) -> Result<Vec<u8>> {
    let cipher = Cipher::new(token_cache_key(key)?);
    let nonce = nonce::random()?;

    let mut bytes = Vec::with_capacity(HEADER_LEN + plaintext.len() + TAG_LEN);
    bytes.extend_from_slice(&MAGIC);
    bytes.push(VERSION);
    bytes.push(0); // reserved
    bytes.extend_from_slice(&nonce);

    // Everything written so far is the associated data, so a file whose magic,
    // version, or nonce was edited fails to open rather than being read as
    // something it is not.
    let associated_data = bytes.clone();
    // The caller's plaintext is a bearer credential, and `seal` needs a buffer
    // it may encrypt in place; this copy of it is wiped rather than left in
    // freed memory.
    let mut buffer = Zeroizing::new(plaintext.to_vec());
    cipher.seal(&nonce, &associated_data, &mut buffer, &mut bytes)?;
    Ok(bytes)
}
