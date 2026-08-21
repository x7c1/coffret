use std::fs;
use std::io;
use std::path::Path;

/// The permissions the log directory is kept at.
#[cfg(unix)]
pub(super) const OWNER_ONLY_DIRECTORY: u32 = 0o700;

/// Creates the directory, owner-only, if it is not already there.
///
/// The mode is applied as each missing directory is created, and a directory
/// that already exists is left exactly as it was found. That second half
/// matters because the directory is a setting: `COFFRET_LOG_DIR` may name one
/// that is somebody's home or a shared temporary directory, and tightening the
/// permissions of a directory coffret did not make would break whatever else
/// lives in it.
#[cfg(unix)]
pub(super) fn create_directory(directory: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    fs::DirBuilder::new()
        .recursive(true)
        .mode(OWNER_ONLY_DIRECTORY)
        .create(directory)
}

/// Creates the directory where owner-only permissions have no meaning.
#[cfg(not(unix))]
pub(super) fn create_directory(directory: &Path) -> io::Result<()> {
    fs::create_dir_all(directory)
}
