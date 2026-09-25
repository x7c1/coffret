use std::path::{Path, PathBuf};
use std::sync::Arc;

use coffret_format::{
    decode_account_token_cache, decode_token_cache, encode_account_token_cache, encode_token_cache,
    Purpose, PurposeKey,
};
use coffret_model::AccountCacheKey;

use crate::error::{Error, Result};

mod load;
mod store;

#[cfg(test)]
mod tests;

/// The file the refresh token is kept in, encrypted.
///
/// The token is a bearer credential for every object this application created
/// in the account: whoever holds it can read and write every object coffret put
/// there, though not open any of them, since Storage only ever sees ciphertext.
/// So the file is sealed (spec: KD-10) — under the account's own account-cache
/// key, which a device keeps one cache per account under (spec: KD-12, SA-8),
/// or, for a Library's previous per-Library cache, under a key derived from
/// that Library's Master Key for that one purpose (spec: KD-4). Which of the
/// two a cache is sealed under is said by the constructor it was made with,
/// and nothing else about it differs. The file is still written with
/// owner-only permissions, since encryption is no reason to hand the bytes to
/// every account on the machine.
///
/// A write replaces the file by renaming a temporary neighbour over it, so a
/// run that dies mid-write leaves the grant that was cached rather than a
/// truncated file. That matters because renewing a grant is an ordinary thing a
/// device does while the old one still works, and an interrupted renewal should
/// cost nothing rather than one trip to a browser.
///
/// What arrives here is the one key that opens the cache and never a Master
/// Key: the gateway holds exactly what it needs and no more. The key is behind
/// an `Arc` because a grant's flow and its refresh both work from the same
/// cache and the type is cloned to reach them — the handle is what is copied,
/// and the key material stays in one place that wipes itself when the last
/// handle goes (spec: DK-7).
///
/// A file that does not open — tampered with, truncated, written under another
/// key, or left by a build that wrote the cache in the clear — is reported as
/// [`Error::MalformedTokenCache`] and never treated as "nothing is cached": an
/// unreadable credential store is a fact worth reporting, and what it costs the
/// caller is one run of the authorization flow.
///
/// [`Error::MalformedTokenCache`]: crate::error::Error::MalformedTokenCache
#[derive(Debug, Clone)]
pub struct TokenCache {
    path: PathBuf,
    key: SealingKey,
}

/// The key a cache is sealed under, and so which of the two caches it is.
#[derive(Debug, Clone)]
enum SealingKey {
    /// An account's account-cache key (spec: KD-12).
    Account(Arc<AccountCacheKey>),
    /// A Library's `coffret/v1/token-cache` purpose key, for its previous
    /// per-Library cache (spec: KD-4).
    Library(Arc<PurposeKey>),
}

/// The permissions the cache file is kept at: readable and writable by its
/// owner, and by nobody else.
#[cfg(unix)]
const OWNER_ONLY: u32 = 0o600;

impl TokenCache {
    /// Points at the file the tokens are kept in, and the key that seals them.
    ///
    /// The key is the one derived for `coffret/v1/token-cache`; any other is
    /// refused as [`Error::WrongTokenCacheKey`] rather than used (spec: KD-4),
    /// before the file it was brought for is read or written.
    ///
    /// [`Error::WrongTokenCacheKey`]: crate::error::Error::WrongTokenCacheKey
    pub fn new(path: impl Into<PathBuf>, key: Arc<PurposeKey>) -> Self {
        Self {
            path: path.into(),
            key: SealingKey::Library(key),
        }
    }

    /// Points at an account's cache, and the account-cache key that seals it
    /// (spec: KD-10, KD-12).
    ///
    /// The key is drawn at random rather than derived, so there is no purpose to
    /// check it against: the type is the whole of what says which key it is.
    pub fn for_account(path: impl Into<PathBuf>, key: Arc<AccountCacheKey>) -> Self {
        Self {
            path: path.into(),
            key: SealingKey::Account(key),
        }
    }

    /// The file the tokens are kept in.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Refuses a key that was derived for some other purpose (spec: KD-4).
    ///
    /// The format layer checks this too, and has to: the rule is that layer's.
    /// What asking here first buys is which of two things a caller is told
    /// happened — a key that is not this cache's is not a cache that cannot be
    /// read, and only one of the two is answered by authorizing again.
    fn require_own_key(&self) -> Result<()> {
        let SealingKey::Library(key) = &self.key else {
            return Ok(());
        };
        let actual = key.purpose();
        if actual == Purpose::TokenCache {
            return Ok(());
        }
        Err(Error::WrongTokenCacheKey {
            path: self.path.clone(),
            actual,
        })
    }

    /// The sealed form of `document`, under this cache's key.
    fn seal(&self, document: &[u8]) -> coffret_format::Result<Vec<u8>> {
        match &self.key {
            SealingKey::Account(key) => encode_account_token_cache(document, key),
            SealingKey::Library(key) => encode_token_cache(document, key),
        }
    }

    /// The document `bytes` seal, under this cache's key.
    fn open(&self, bytes: &[u8]) -> coffret_format::Result<Vec<u8>> {
        match &self.key {
            SealingKey::Account(key) => decode_account_token_cache(bytes, key),
            SealingKey::Library(key) => decode_token_cache(bytes, key),
        }
    }
}
