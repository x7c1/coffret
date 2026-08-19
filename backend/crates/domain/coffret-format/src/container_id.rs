use coffret_model::ContainerId;

use crate::entropy;
use crate::error::Result;

/// Draws a fresh Container ID from the operating system's CSPRNG.
///
/// The generator takes no input from the content the Container will hold, which
/// is what makes the object name derived from the ID say nothing about it.
pub fn generate_container_id() -> Result<ContainerId> {
    Ok(ContainerId::from_bytes(entropy::draw()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // FM-3: the Container ID is 128 bits drawn from a CSPRNG.
    #[test]
    fn draws_distinct_128_bit_identifiers() {
        let ids: HashSet<ContainerId> = (0..256)
            .map(|_| generate_container_id().expect("the OS CSPRNG is available"))
            .collect();
        assert_eq!(ids.len(), 256);
        for id in &ids {
            assert_eq!(id.as_bytes().len(), 16);
        }
    }
}
