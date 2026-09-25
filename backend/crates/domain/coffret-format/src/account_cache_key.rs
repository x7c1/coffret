use coffret_model::AccountCacheKey;

use crate::entropy;
use crate::error::Result;

/// Draws a fresh account-cache key from the operating system's CSPRNG
/// (spec: KD-12).
///
/// Called once, when a device first keeps a grant for an account. It takes
/// nothing in: the key is derived from no Master Key (see [`AccountCacheKey`]).
pub fn generate_account_cache_key() -> Result<AccountCacheKey> {
    Ok(AccountCacheKey::from_bytes(entropy::draw()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // KD-12: an account-cache key is 256 bits drawn from the operating system's
    // CSPRNG, one per account a device keeps a grant for.
    #[test]
    fn draws_distinct_256_bit_keys() {
        let keys: HashSet<[u8; AccountCacheKey::BYTE_LEN]> = (0..256)
            .map(|_| {
                *generate_account_cache_key()
                    .expect("the OS CSPRNG is available")
                    .as_bytes()
            })
            .collect();
        assert_eq!(keys.len(), 256);
        for key in &keys {
            assert_eq!(key.len(), 32);
        }
    }
}
