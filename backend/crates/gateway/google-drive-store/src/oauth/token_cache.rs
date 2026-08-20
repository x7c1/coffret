use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::oauth::stored_tokens::StoredTokens;

/// The file the refresh token is kept in.
///
/// The token is a bearer credential for the Library's Storage: whoever holds it
/// can read and write every object coffret put there, though not open any of
/// them, since Storage only ever sees ciphertext. It is written with owner-only
/// permissions.
///
/// It is not yet encrypted at rest — protecting it under a Master-Key-derived
/// key first needs a purpose of its own in the key-derivation registry
/// (spec: KD-4), since a key derived for one purpose is used for no other.
/// Until then the file permissions are the whole of its protection, which is
/// why they are set explicitly on every write rather than left to the process
/// umask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenCache {
    path: PathBuf,
}

/// The permissions the cache file is kept at: readable and writable by its
/// owner, and by nobody else.
#[cfg(unix)]
const OWNER_ONLY: u32 = 0o600;

impl TokenCache {
    /// Points at the file the tokens are kept in.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The file the tokens are kept in.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads the cached tokens, or `None` if nothing has been cached yet.
    pub fn load(&self) -> Result<Option<StoredTokens>> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(Error::TokenCache {
                    path: self.path.clone(),
                    detail: error.to_string(),
                })
            }
        };

        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| Error::MalformedTokenCache {
                path: self.path.clone(),
                detail: error.to_string(),
            })
    }

    /// Writes the tokens, replacing whatever was cached before.
    pub fn store(&self, tokens: &StoredTokens) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| Error::TokenCache {
                path: self.path.clone(),
                detail: error.to_string(),
            })?;
        }

        let document = serde_json::to_vec(tokens).map_err(|error| Error::TokenCache {
            path: self.path.clone(),
            detail: error.to_string(),
        })?;

        self.write_owner_only(&document)
    }

    /// Writes the file, owner-only from the moment it exists.
    #[cfg(unix)]
    fn write_owner_only(&self, document: &[u8]) -> Result<()> {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let describe = |error: std::io::Error| Error::TokenCache {
            path: self.path.clone(),
            detail: error.to_string(),
        };

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            // Applies when this call creates the file, which is the case that
            // matters: the token must never exist as a world-readable file, not
            // even for the instant before a `chmod`.
            .mode(OWNER_ONLY)
            .open(&self.path)
            .map_err(describe)?;

        // And again for a file that already existed, whose mode the open above
        // leaves alone.
        file.set_permissions(fs::Permissions::from_mode(OWNER_ONLY))
            .map_err(describe)?;

        file.write_all(document).map_err(describe)?;
        file.sync_all().map_err(describe)
    }

    /// Writes the file where owner-only permissions have no meaning.
    #[cfg(not(unix))]
    fn write_owner_only(&self, document: &[u8]) -> Result<()> {
        fs::write(&self.path, document).map_err(|error| Error::TokenCache {
            path: self.path.clone(),
            detail: error.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_cache_reads_as_nothing_cached() {
        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        let cache = TokenCache::new(directory.path().join("tokens.json"));

        assert_eq!(cache.load().expect("a missing file is not an error"), None);
    }

    #[test]
    fn what_is_stored_is_what_is_loaded() {
        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        let cache = TokenCache::new(directory.path().join("nested/tokens.json"));
        let tokens = StoredTokens {
            refresh_token: "1//refresh".to_owned(),
        };

        cache.store(&tokens).expect("storing must succeed");
        assert_eq!(cache.load().expect("loading must succeed"), Some(tokens));
    }

    #[cfg(unix)]
    #[test]
    fn the_cache_is_readable_by_its_owner_and_nobody_else() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        let cache = TokenCache::new(directory.path().join("tokens.json"));
        cache
            .store(&StoredTokens {
                refresh_token: "1//refresh".to_owned(),
            })
            .expect("storing must succeed");

        let mode = fs::metadata(cache.path())
            .expect("the file must exist")
            .permissions()
            .mode();

        assert_eq!(mode & 0o777, OWNER_ONLY);
    }

    #[cfg(unix)]
    #[test]
    fn a_loosely_permissioned_cache_is_tightened_on_the_next_write() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        let path = directory.path().join("tokens.json");
        fs::write(&path, b"{}").expect("the file must be writable");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .expect("permissions must be settable");

        TokenCache::new(&path)
            .store(&StoredTokens {
                refresh_token: "1//refresh".to_owned(),
            })
            .expect("storing must succeed");

        let mode = fs::metadata(&path)
            .expect("the file must exist")
            .permissions()
            .mode();

        assert_eq!(mode & 0o777, OWNER_ONLY);
    }
}
