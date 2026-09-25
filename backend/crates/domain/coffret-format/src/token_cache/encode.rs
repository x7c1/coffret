use coffret_model::AccountCacheKey;
use zeroize::Zeroizing;

use super::{token_cache_key, HEADER_LEN, MAGIC, VERSION};
use crate::aead::{Cipher, KEY_LEN, TAG_LEN};
use crate::error::Result;
use crate::nonce;
use crate::purpose_key::PurposeKey;

/// Seals a Library's per-Library token cache under the token-cache purpose key.
///
/// The previous shape (spec: KD-10): a device no longer writes a new cache in
/// it, and it stays for the promotion's tests and for the tools that still
/// reach a Library's grant that way. A key derived for any other purpose is
/// refused rather than used (KD-4).
pub fn encode_token_cache(plaintext: &[u8], key: &PurposeKey) -> Result<Vec<u8>> {
    seal(plaintext, token_cache_key(key)?)
}

/// Seals an account's token cache under its account-cache key
/// (spec: KD-10, KD-12).
pub fn encode_account_token_cache(plaintext: &[u8], key: &AccountCacheKey) -> Result<Vec<u8>> {
    seal(plaintext, key.as_bytes())
}

/// The form's writing, under whichever key the caller was entitled to bring.
///
/// The nonce is drawn fresh on every call: one key covers every write of a
/// cache, renewals included, so nothing but a random nonce keeps two writes
/// from sharing one. The bytes returned are the whole file — magic, version,
/// nonce, and the AEAD message — and the caller writes them as they are.
fn seal(plaintext: &[u8], key: &[u8; KEY_LEN]) -> Result<Vec<u8>> {
    let cipher = Cipher::new(key);
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
