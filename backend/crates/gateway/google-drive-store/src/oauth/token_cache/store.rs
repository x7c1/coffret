use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::TokenCache;
use crate::error::{Error, Result};
use crate::oauth::stored_tokens::StoredTokens;

impl TokenCache {
    /// Writes the tokens, replacing whatever was cached before.
    ///
    /// The replacement is a rename over a temporary neighbour, for the reason
    /// [`TokenCache`] gives: an interrupted write leaves the grant that was
    /// cached rather than a truncated file.
    pub fn store(&self, tokens: &StoredTokens) -> Result<()> {
        // Asked before anything is created, for the reason `load` asks it: a
        // key derived for another purpose is the caller's mistake rather than a
        // cache that could not be sealed, and no directory should come into
        // being over one.
        self.require_own_key()?;

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
        let sealed = self
            .seal(&document)
            .map_err(|cause| Error::UnsealableTokenCache {
                path: self.path.clone(),
                cause,
            })?;

        replace(&self.path, &self.temporary_neighbour(), |file| {
            use std::io::Write;

            file.write_all(&sealed)?;
            file.sync_all()
        })
    }

    /// A name in the same directory nothing else is using.
    ///
    /// The same directory, because a rename is only atomic within one
    /// filesystem and the whole point of the temporary file is that the
    /// replacement either happens or does not.
    ///
    /// The process id and a sequence keep this process's writes apart; the
    /// random part keeps it apart from every other run's. A run that crashed
    /// mid-write leaves its neighbour behind, and the operating system hands
    /// its process id out again: without the random part a later run under
    /// that id would reach the same sequence, find the name taken, and fail to
    /// store a grant every time until somebody deleted the file by hand.
    fn temporary_neighbour(&self) -> PathBuf {
        let sequence = NEXT_NEIGHBOUR.fetch_add(1, Ordering::Relaxed);

        let mut random = [0_u8; 8];
        // Nothing rests on the name being unpredictable, so an entropy source
        // that refuses is no reason to refuse the write: the name falls back to
        // the process id and sequence alone.
        let _ = getrandom::fill(&mut random);
        let random = u64::from_ne_bytes(random);

        let name = self
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();

        self.path.with_file_name(format!(
            ".{name}.{}-{sequence}-{random:016x}.tmp",
            std::process::id()
        ))
    }
}

/// How many temporary neighbours this process has named, so that no two of its
/// own writes pick the same name.
pub(super) static NEXT_NEIGHBOUR: AtomicU64 = AtomicU64::new(0);

/// Creates `temporary`, lets `fill` write it, and renames it over `path`.
///
/// The neighbour is this call's own litter from the moment it exists, so every
/// way out after that — `fill` failing as much as the rename — removes it:
/// leaving one would be a second copy of the sealed grant, lying beside the
/// cache under a name nothing reads.
///
/// A refusal to create or fill the neighbour names `temporary` and not the
/// cache it is on its way to becoming: the neighbour is what the operating
/// system would not create or write, and a message naming the cache instead
/// would report a file existing as the reason a file could not be created.
pub(super) fn replace(
    path: &Path,
    temporary: &Path,
    fill: impl FnOnce(&mut fs::File) -> std::io::Result<()>,
) -> Result<()> {
    let describe = |path: &Path| {
        let path = path.to_path_buf();
        move |cause: std::io::Error| Error::TokenCache { path, cause }
    };

    let mut file = create_owner_only(temporary).map_err(describe(temporary))?;
    let filled = fill(&mut file).map_err(describe(temporary));
    drop(file);

    let outcome = filled.and_then(|()| fs::rename(temporary, path).map_err(describe(path)));
    if outcome.is_err() {
        // The failure being reported is the one that matters; a neighbour that
        // will not go either is left behind, where it blocks nobody: no two
        // runs pick the same name.
        let _ = fs::remove_file(temporary);
    }
    outcome
}

/// Creates a file that is not there, owner-only from the moment it exists.
fn create_owner_only(path: &Path) -> std::io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Applies as this call creates the file, which is every time now that
        // the write goes to a fresh neighbour: the cache must never exist as a
        // world-readable file, not even for the instant before a `chmod`.
        options.mode(super::OWNER_ONLY);
    }

    options.open(path)
}
