use std::fs::{self, File};
use std::io;
use std::path::Path;

use time::OffsetDateTime;

use super::files::Current;
use super::{FILE_PREFIX, FILE_SUFFIX};

/// The permissions a log file is kept at: readable and writable by its owner,
/// and by nobody else.
///
/// The log is evidence of what a Library's Storage answered, and although it
/// carries no key material and no Entry Path, it is nobody else's business —
/// the same reasoning, and the same mode, as the token cache.
#[cfg(unix)]
pub(super) const OWNER_ONLY_FILE: u32 = 0o600;

/// How many names are tried before giving up on starting a file.
///
/// Names carry a timestamp to the second and a sequence number, so a collision
/// means another process started a file in the same second; a handful of tries
/// settles it.
const NAME_ATTEMPTS: u32 = 1000;

/// Starts a file nothing else is writing to.
pub(super) fn start_file(directory: &Path) -> io::Result<Current> {
    let stamp = timestamp();
    for sequence in 0..NAME_ATTEMPTS {
        let path = directory.join(format!("{FILE_PREFIX}{stamp}-{sequence:03}{FILE_SUFFIX}"));
        match create_owner_only(&path) {
            Ok(file) => return Ok(Current { path, file, len: 0 }),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("every log file name for {stamp} is taken"),
    ))
}

/// Creates a file that never exists as a readable one to anybody else.
#[cfg(unix)]
fn create_owner_only(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    fs::OpenOptions::new()
        .append(true)
        .create_new(true)
        // Applied as the file is created, so it is never world-readable — not
        // even for the instant before a `chmod`.
        .mode(OWNER_ONLY_FILE)
        .open(path)
}

/// Creates a file where owner-only permissions have no meaning.
#[cfg(not(unix))]
fn create_owner_only(path: &Path) -> io::Result<File> {
    fs::OpenOptions::new()
        .append(true)
        .create_new(true)
        .open(path)
}

/// The moment a file was started, as its name records it.
///
/// UTC and fixed-width, so the directory listing reads in the order the files
/// were written and a name means the same thing wherever it is read.
fn timestamp() -> String {
    let now = OffsetDateTime::now_utc();
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}
