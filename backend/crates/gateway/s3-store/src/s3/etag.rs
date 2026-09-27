use coffret_usecase::ProviderHash;

/// Reads an ETag S3 answered with as the digest the port speaks.
///
/// S3 quotes its ETags; the quotes are transport syntax, not part of the
/// digest, and leaving them in would make the value fail to compare against
/// anything computed locally. One reading for both places an ETag arrives — a
/// listing and the answer to a write — so the two cannot spell one digest two
/// ways.
pub(crate) fn provider_hash(tag: &str) -> ProviderHash {
    ProviderHash::new(tag.trim_matches('"'))
}
