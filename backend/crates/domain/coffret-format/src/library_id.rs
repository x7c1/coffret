use coffret_model::LibraryId;

use crate::entropy;
use crate::error::Result;

/// Draws a fresh Library ID from the operating system's CSPRNG (spec: FM-18).
///
/// The same source every other identifier and key comes from, and deliberately
/// not the Master Key: the app folder named after this ID must keep its name
/// across a rotation, and a name Storage can read must not be derived from key
/// material.
pub fn generate_library_id() -> Result<LibraryId> {
    Ok(LibraryId::from_bytes(entropy::draw()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    // FM-18: the Library ID is 64 bits drawn from a CSPRNG, so two Libraries
    // created moments apart are named apart.
    #[test]
    fn draws_distinct_64_bit_identifiers() {
        let first = generate_library_id().expect("the OS CSPRNG is available");
        let second = generate_library_id().expect("the OS CSPRNG is available");

        assert_ne!(first, second);
        assert_eq!(first.as_bytes().len(), 8);
        assert_eq!(second.as_bytes().len(), 8);
    }
}
