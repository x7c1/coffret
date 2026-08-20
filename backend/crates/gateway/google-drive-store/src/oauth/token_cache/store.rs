use std::fs;

use coffret_format::encode_token_cache;

use super::TokenCache;
use crate::error::{Error, Result};
use crate::oauth::stored_tokens::StoredTokens;

impl TokenCache {
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

        // Whatever kept the format layer from sealing travels with the error
        // rather than being read as one particular cause: what this layer knows
        // is that the cache could not be sealed, and so was not written.
        let sealed = encode_token_cache(&document, &self.master_key).map_err(|cause| {
            Error::UnsealableTokenCache {
                path: self.path.clone(),
                cause,
            }
        })?;

        self.write_owner_only(&sealed)
    }

    /// Writes the file, owner-only from the moment it exists.
    #[cfg(unix)]
    fn write_owner_only(&self, document: &[u8]) -> Result<()> {
        use std::io::Write;
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
            // matters: the cache must never exist as a world-readable file, not
            // even for the instant before a `chmod`.
            .mode(super::OWNER_ONLY)
            .open(&self.path)
            .map_err(describe)?;

        // And again for a file that already existed, whose mode the open above
        // leaves alone.
        file.set_permissions(fs::Permissions::from_mode(super::OWNER_ONLY))
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
