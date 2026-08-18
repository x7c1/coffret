use coffret_model::ContainerKey;

use crate::entropy;
use crate::error::Result;

/// Draws a fresh Container Key from the operating system's CSPRNG.
///
/// Each Container Key is drawn independently and is never derived from the
/// Master Key, so no derivation path exists from one Container's key to
/// another's. Independent keys are what let one Container be replaced or
/// discarded without re-keying any other, and keep a future single-Container
/// sharing path open.
pub fn generate_container_key() -> Result<ContainerKey> {
    Ok(ContainerKey::from_bytes(entropy::draw()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // KD-2: each Container Key is 256 bits drawn independently from a CSPRNG,
    // never shared between Containers.
    #[test]
    fn draws_distinct_256_bit_keys() {
        let keys: HashSet<[u8; ContainerKey::BYTE_LEN]> = (0..256)
            .map(|_| {
                *generate_container_key()
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
