use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use coffret_format::encode_token_cache;

use super::TokenCache;
use crate::error::{Error, Result};
use crate::oauth::stored_tokens::StoredTokens;

impl TokenCache {
    /// Writes the tokens, replacing whatever was cached before.
    ///
    /// The replacement happens as a rename over a temporary neighbour, so the
    /// file is either the grant that was cached or the grant that has just been
    /// obtained and never something half-written in between. What that buys is
    /// the case it is written for: authorizing again over a cache that is still
    /// good — the ordinary way a grant is renewed before it expires — costs
    /// nothing when it is interrupted, because what was there is still whole.
    pub fn store(&self, tokens: &StoredTokens) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|cause| Error::TokenCache {
                path: self.path.clone(),
                cause,
            })?;
        }

        let document = serde_json::to_vec(tokens).map_err(|cause| Error::UnencodableTokens {
            path: self.path.clone(),
            cause,
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

        let temporary = self.temporary_neighbour();
        Self::write_owner_only(&temporary, &sealed)?;

        match fs::rename(&temporary, &self.path) {
            Ok(()) => Ok(()),
            Err(cause) => {
                // The neighbour is this call's own litter, and leaving it would
                // make the next attempt fail for a reason of its own.
                let _ = fs::remove_file(&temporary);
                Err(Error::TokenCache {
                    path: self.path.clone(),
                    cause,
                })
            }
        }
    }

    /// A name in the same directory nothing else is using.
    ///
    /// The same directory, because a rename is only atomic within one
    /// filesystem and the whole point of the temporary file is that the
    /// replacement either happens or does not.
    fn temporary_neighbour(&self) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, Ordering::Relaxed);

        let name = self
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();

        self.path
            .with_file_name(format!(".{name}.{}-{sequence}.tmp", std::process::id()))
    }

    /// Writes the file, owner-only from the moment it exists.
    ///
    /// A refusal names `path` and not the cache it is on its way to becoming:
    /// the neighbour is what the operating system would not create or write,
    /// and a message naming the cache instead would report a file existing as
    /// the reason a file could not be created.
    #[cfg(unix)]
    fn write_owner_only(path: &Path, document: &[u8]) -> Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let describe = |cause: std::io::Error| Error::TokenCache {
            path: path.to_path_buf(),
            cause,
        };

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            // Applies as this call creates the file, which is every time now
            // that the write goes to a fresh neighbour: the cache must never
            // exist as a world-readable file, not even for the instant before a
            // `chmod`.
            .mode(super::OWNER_ONLY)
            .open(path)
            .map_err(describe)?;

        file.write_all(document).map_err(describe)?;
        file.sync_all().map_err(describe)
    }

    /// Writes the file where owner-only permissions have no meaning.
    #[cfg(not(unix))]
    fn write_owner_only(path: &Path, document: &[u8]) -> Result<()> {
        fs::write(path, document).map_err(|cause| Error::TokenCache {
            path: path.to_path_buf(),
            cause,
        })
    }
}
