use std::path::{Path, PathBuf};

use coffret_model::MasterKey;

mod load;
mod store;

#[cfg(test)]
mod tests;

/// The file the refresh token is kept in, encrypted.
///
/// The token is a bearer credential for the Library's Storage: whoever holds it
/// can read and write every object coffret put there, though not open any of
/// them, since Storage only ever sees ciphertext. So the file is sealed under a
/// key derived from the Master Key for that one purpose (spec: KD-4), which is
/// the same protection everything else coffret writes gets — and the file is
/// still written with owner-only permissions, since encryption is no reason to
/// hand the bytes to every account on the machine.
///
/// A write replaces the file by renaming a temporary neighbour over it, so a
/// run that dies mid-write leaves the grant that was cached rather than a
/// truncated file. That matters because renewing a grant is an ordinary thing a
/// device does while the old one still works, and an interrupted renewal should
/// cost nothing rather than one trip to a browser.
///
/// The Master Key arrives here already unlocked, and `coffret-format` derives
/// the token-cache key from it on each call, so no derived key is ever held or
/// passed around in the gateway.
///
/// A file that does not open — tampered with, truncated, written under another
/// Master Key, or left by a build that wrote the cache in the clear — is
/// reported as [`Error::MalformedTokenCache`] and never treated as "nothing is
/// cached": an unreadable credential store is a fact worth reporting, and what
/// it costs the caller is one run of the authorization flow.
///
/// [`Error::MalformedTokenCache`]: crate::error::Error::MalformedTokenCache
#[derive(Debug, Clone)]
pub struct TokenCache {
    path: PathBuf,
    master_key: MasterKey,
}

/// The permissions the cache file is kept at: readable and writable by its
/// owner, and by nobody else.
#[cfg(unix)]
const OWNER_ONLY: u32 = 0o600;

impl TokenCache {
    /// Points at the file the tokens are kept in, and the key that seals them.
    pub fn new(path: impl Into<PathBuf>, master_key: MasterKey) -> Self {
        Self {
            path: path.into(),
            master_key,
        }
    }

    /// The file the tokens are kept in.
    pub fn path(&self) -> &Path {
        &self.path
    }
}
