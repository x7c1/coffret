use coffret_model::MasterKey;

use crate::entropy;
use crate::error::Result;

/// Draws a fresh Master Key from the operating system's CSPRNG.
///
/// The generator takes no user input at all — in particular not the Passphrase
/// — so the strength of the ciphertext on Storage never depends on passphrase
/// quality. Each Master Key epoch calls this again for a key of its own.
pub fn generate_master_key() -> Result<MasterKey> {
    Ok(MasterKey::from_bytes(entropy::draw()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // KD-1: the Master Key is 256 bits drawn from the OS CSPRNG, and each epoch
    // draws its own.
    #[test]
    fn draws_distinct_256_bit_keys() {
        let keys: HashSet<[u8; MasterKey::BYTE_LEN]> = (0..256)
            .map(|_| {
                *generate_master_key()
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
