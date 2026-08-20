use coffret_model::MasterKey;

use super::{token_cache_key, Layout};
use crate::aead::Cipher;
use crate::error::Result;

/// Opens a sealed token cache with the Master Key that sealed it.
///
/// A cache that fails its shape check or its authentication yields an error and
/// never a plaintext: a file written under another Master Key, tampered with,
/// truncated, or left behind by another tool is a fact for the caller to act on,
/// not a cache to be treated as empty.
pub fn decode_token_cache(bytes: &[u8], master_key: &MasterKey) -> Result<Vec<u8>> {
    let layout = Layout::parse(bytes)?;

    // The associated data is everything ahead of the message, exactly as it
    // appears in the file.
    Cipher::new(&token_cache_key(master_key)).open(
        &layout.nonce,
        &bytes[..layout.message.start],
        &bytes[layout.message.clone()],
    )
}
