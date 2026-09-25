use coffret_model::AccountCacheKey;

use super::{token_cache_key, Layout};
use crate::aead::{Cipher, KEY_LEN};
use crate::error::Result;
use crate::purpose_key::PurposeKey;

/// Opens a Library's previous per-Library token cache with the purpose key that
/// sealed it (spec: KD-10).
///
/// A cache that fails its shape check or its authentication yields an error and
/// never a plaintext: a file written under another Master Key, tampered with,
/// truncated, or left behind by another tool is a fact for the caller to act on,
/// not a cache to be treated as empty. A key derived for another purpose is
/// refused before any of that (KD-4).
pub fn decode_token_cache(bytes: &[u8], key: &PurposeKey) -> Result<Vec<u8>> {
    open(bytes, token_cache_key(key)?)
}

/// Opens an account's token cache with the account-cache key that sealed it
/// (spec: KD-10, KD-12).
///
/// Refused as [`decode_token_cache`] refuses: a cache that fails its shape
/// check or its authentication — sealed under another account's key among
/// them — yields an error and never a plaintext.
pub fn decode_account_token_cache(bytes: &[u8], key: &AccountCacheKey) -> Result<Vec<u8>> {
    open(bytes, key.as_bytes())
}

/// The form's reading, under whichever key the caller was entitled to bring.
fn open(bytes: &[u8], key: &[u8; KEY_LEN]) -> Result<Vec<u8>> {
    let cipher = Cipher::new(key);
    let layout = Layout::parse(bytes)?;

    // The associated data is everything ahead of the message, exactly as it
    // appears in the file.
    cipher.open(
        &layout.nonce,
        &bytes[..layout.message.start],
        &bytes[layout.message.clone()],
    )
}
