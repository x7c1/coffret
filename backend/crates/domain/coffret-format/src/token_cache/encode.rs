use coffret_model::MasterKey;

use super::{token_cache_key, HEADER_LEN, MAGIC, VERSION};
use crate::aead::{Cipher, TAG_LEN};
use crate::error::Result;
use crate::nonce;

/// Seals a device's token cache under the Master Key.
///
/// The nonce is drawn fresh on every call: one key covers every cache this
/// device ever writes, so nothing but a random nonce keeps two writes from
/// sharing one. The bytes returned are the whole file — magic, version, nonce,
/// and the AEAD message — and the caller writes them as they are.
pub fn encode_token_cache(plaintext: &[u8], master_key: &MasterKey) -> Result<Vec<u8>> {
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
    let mut buffer = plaintext.to_vec();
    Cipher::new(&token_cache_key(master_key)).seal(
        &nonce,
        &associated_data,
        &mut buffer,
        &mut bytes,
    )?;
    Ok(bytes)
}
