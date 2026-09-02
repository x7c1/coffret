//! Writing a device's own files so that only their owner can read them.
//!
//! Three of the six things a Library directory holds are worth nobody else's
//! account on the machine reading: the stored Master Key, the sealed grant, and
//! the catalog — which is plaintext, and is the one file that names Entry Paths.
//! The settings file joins them because it carries the OAuth client secret for
//! a Drive Library, and the running server's key joins them because whoever can
//! read it can ask that server for the Library's plaintext. So all of them are
//! created owner-only, from the moment they exist rather than by a `chmod` after
//! the fact.
//!
//! A file is written to a temporary neighbour and renamed over the target, so a
//! run that dies mid-write leaves the previous contents rather than a truncated
//! file. That matters most for the grant: a device whose token cache was
//! truncated by an interrupted write would have to authorize again, and the
//! whole point of the cache is that it does not.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{Error, Result};

/// The permissions a device's own files are kept at: readable and writable by
/// their owner, and by nobody else.
#[cfg(unix)]
pub(crate) const OWNER_ONLY_FILE: u32 = 0o600;

/// The same for a directory, which needs the execute bit to be entered at all.
#[cfg(unix)]
pub(crate) const OWNER_ONLY_DIRECTORY: u32 = 0o700;

/// Writes `bytes` to `path`, owner-only, replacing whatever was there.
pub(crate) fn write_file(doing: &'static str, path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = temporary_neighbour(path);
    create(doing, &temporary, bytes)?;

    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(cause) => {
            // The half-written neighbour is this call's own litter, and leaving
            // it would make the next attempt's `create_new` fail for a reason
            // that has nothing to do with the next attempt.
            let _ = fs::remove_file(&temporary);
            Err(Error::Local {
                doing,
                path: path.to_path_buf(),
                cause,
            })
        }
    }
}

/// Creates a directory and everything above it, owner-only.
pub(crate) fn create_dir(doing: &'static str, path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(OWNER_ONLY_DIRECTORY);
    }

    builder.create(path).map_err(Error::local(doing, path))
}

/// Creates an empty file, owner-only, refusing to touch one that is there.
///
/// SQLite is happy to open a zero-length file as an empty database, which is
/// what lets the catalog exist at the right mode from the moment it exists
/// rather than at whatever the process umask would have given it.
pub(crate) fn create_empty_file(doing: &'static str, path: &Path) -> Result<()> {
    create(doing, path, &[])
}

/// Creates a file that is not there and writes `bytes` into it.
fn create(doing: &'static str, path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Applies as the file is created, which is the case that matters: none
        // of these files may exist as a world-readable file, not even for the
        // instant before a `chmod`.
        options.mode(OWNER_ONLY_FILE);
    }

    let mut file = options.open(path).map_err(Error::local(doing, path))?;
    file.write_all(bytes).map_err(Error::local(doing, path))?;
    file.sync_all().map_err(Error::local(doing, path))
}

/// A name in the same directory nothing else is using.
///
/// The same directory, because a rename is only atomic within one filesystem,
/// and the point of the temporary file is that the rename either happens or
/// does not.
fn temporary_neighbour(path: &Path) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);

    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();

    path.with_file_name(format!(".{name}.{}-{sequence}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_file_holds_what_was_written() {
        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        let path = directory.path().join("settings.json");

        write_file("writing", &path, b"first").expect("the first write must land");
        write_file("writing", &path, b"second").expect("a rewrite must replace it");

        assert_eq!(
            fs::read(&path).expect("the file must be readable"),
            b"second"
        );
    }

    // The mode is the protection the OAuth client secret and the plaintext
    // catalog rest on, so it is asserted rather than assumed.
    #[cfg(unix)]
    #[test]
    fn a_written_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        let path = directory.path().join("master-key.cfmk");
        write_file("writing", &path, b"bytes").expect("the write must land");

        let mode = fs::metadata(&path)
            .expect("the file must be there")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, OWNER_ONLY_FILE);
    }

    // A rename leaves no litter behind, including on the run that made it.
    #[test]
    fn nothing_is_left_beside_a_written_file() {
        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        write_file("writing", &directory.path().join("settings.json"), b"{}")
            .expect("the write must land");

        let names: Vec<_> = fs::read_dir(directory.path())
            .expect("the directory must be readable")
            .map(|entry| entry.expect("an entry must be readable").file_name())
            .collect();
        assert_eq!(names, ["settings.json"]);
    }
}
