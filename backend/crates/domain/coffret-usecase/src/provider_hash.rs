use std::fmt;

/// The digest a Storage provider reports for the bytes it holds.
///
/// Providers do not agree on the algorithm — Drive reports MD5, S3 an ETag that
/// is an MD5 only for single-part uploads — so this is a provider-scoped token,
/// good for asking "are these the bytes I uploaded" against the same provider
/// and for nothing else. End-to-end integrity is
/// [`ContentHash`](coffret_model::ContentHash), which is BLAKE3 over an Entry's
/// plaintext and is verified after decryption.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderHash(String);

impl ProviderHash {
    /// Takes the digest as the provider spells it.
    pub fn new(digest: impl Into<String>) -> Self {
        Self(digest.into())
    }

    /// The digest as the provider spells it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
