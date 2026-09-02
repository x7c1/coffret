use super::{token_cache_key, Layout};
use crate::aead::Cipher;
use crate::error::Result;
use crate::purpose_key::PurposeKey;

/// Opens a sealed token cache with the purpose key that sealed it.
///
/// A cache that fails its shape check or its authentication yields an error and
/// never a plaintext: a file written under another Master Key, tampered with,
/// truncated, or left behind by another tool is a fact for the caller to act on,
/// not a cache to be treated as empty. A key derived for another purpose is
/// refused before any of that (KD-4).
pub fn decode_token_cache(bytes: &[u8], key: &PurposeKey) -> Result<Vec<u8>> {
    let cipher = Cipher::new(token_cache_key(key)?);
    let layout = Layout::parse(bytes)?;

    // The associated data is everything ahead of the message, exactly as it
    // appears in the file.
    cipher.open(
        &layout.nonce,
        &bytes[..layout.message.start],
        &bytes[layout.message.clone()],
    )
}
