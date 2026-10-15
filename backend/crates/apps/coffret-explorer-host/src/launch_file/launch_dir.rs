use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::net::{Ipv4Addr, SocketAddr};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use zeroize::Zeroizing;

use super::{content, LaunchFile, LaunchFileError, DIRECTORY_MODE, FILE_MODE};
use crate::credentials::Credentials;

/// The bits of a mode that grant anything to the group or to others.
const OTHERS: u32 = 0o077;

/// The owner-only directory launch files are written into (spec: LA-21).
#[derive(Debug, Clone)]
pub struct LaunchDir {
    path: PathBuf,
}

impl LaunchDir {
    /// The directory at `path`, created at mode `0700` where it is missing.
    ///
    /// One that is there already is taken only where it is a directory rather
    /// than a link to one, belongs to this process's account, and grants
    /// nothing to anybody else: anyone who can read the directory can read
    /// every ticket put in it.
    pub fn prepare(path: impl Into<PathBuf>) -> Result<Self, LaunchFileError> {
        let path = path.into();
        match fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => DirBuilder::new()
                .recursive(true)
                .mode(DIRECTORY_MODE)
                .create(&path)
                .map_err(LaunchFileError::io("creating the directory"))?,
            Err(cause) => return Err(LaunchFileError::io("inspecting the directory")(cause)),
        }
        check(&path)?;
        Ok(Self { path })
    }

    /// Where the directory is.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Removes every launch file in the directory, and every temporary file
    /// one was being written under, and answers how many went: what an app
    /// that is the directory's only writer does as it starts, since whatever a
    /// run before it left there is of a generation that is over.
    ///
    /// The tickets in such files admit nobody already (spec: LA-17, LA-18);
    /// this is housekeeping, not what keeps them spent. Anything else in the
    /// directory is left where it is. A file that will not go is passed over,
    /// and the first refusal is answered, as [`LaunchFileError::NotRemoved`],
    /// once the rest have been tried.
    pub fn sweep(&self) -> Result<usize, LaunchFileError> {
        check(&self.path)?;
        let entries =
            fs::read_dir(&self.path).map_err(LaunchFileError::io("reading the directory"))?;
        let mut removed = 0;
        let mut refused = None;
        for entry in entries {
            let entry = entry.map_err(LaunchFileError::io("reading the directory"))?;
            let ours = entry.file_name().to_str().is_some_and(is_ours);
            let is_file = entry.file_type().is_ok_and(|kind| kind.is_file());
            if !ours || !is_file {
                continue;
            }
            match fs::remove_file(entry.path()) {
                Ok(()) => removed += 1,
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                Err(cause) => {
                    refused.get_or_insert(cause);
                }
            }
        }
        match refused {
            Some(cause) => Err(LaunchFileError::NotRemoved(cause)),
            None => Ok(removed),
        }
    }

    /// Issues a ticket from `credentials` and writes the file named `name`
    /// that delivers it to the host at `address`, replacing whatever file had
    /// that name.
    ///
    /// The directory is checked again first, since something may have changed
    /// it since it was prepared. The file is written exclusively under a
    /// temporary name of its own at mode `0600` and renamed into place, so a
    /// reader finds the whole of one file or the whole of the one before, and
    /// nobody but the owner finds either.
    pub fn deliver(
        &self,
        name: &str,
        address: SocketAddr,
        credentials: &Credentials,
    ) -> Result<LaunchFile, LaunchFileError> {
        if !is_a_name(name) {
            return Err(LaunchFileError::NotAName);
        }
        if address.ip() != Ipv4Addr::LOCALHOST {
            return Err(LaunchFileError::NotLoopback(address));
        }
        check(&self.path)?;

        let ticket = credentials
            .issue_ticket()
            .map_err(LaunchFileError::NotDrawn)?;
        let target = Zeroizing::new(format!(
            "http://{address}/#launch={}",
            ticket.encoded().as_str()
        ));
        let page = Zeroizing::new(content::page(&target, &credentials.instance_id()));

        let path = self.path.join(name);
        write_exclusively(
            &self.path.join(temporary_name(name)),
            &path,
            page.as_bytes(),
        )?;
        Ok(LaunchFile { path, ticket })
    }
}

/// Whether `name` is one file name of a page: no separator, no dotfile, and
/// ending in `.html`.
fn is_a_name(name: &str) -> bool {
    !name.starts_with('.')
        && !name.contains(['/', '\\', '\0'])
        && name.len() > ".html".len()
        && name.ends_with(".html")
}

/// Whether `name` is one [`LaunchDir::deliver`] writes: a launch file's, or
/// the temporary name one is written under.
fn is_ours(name: &str) -> bool {
    is_a_name(name)
        || name
            .strip_prefix('.')
            .and_then(|rest| rest.strip_suffix(".tmp"))
            .is_some_and(|rest| rest.contains(".html."))
}

/// A hidden name beside `name` that no other write of this process picks. One
/// of another process's that picked it anyway is refused by the exclusive
/// creation rather than written through.
fn temporary_name(name: &str) -> String {
    static WRITES: AtomicU64 = AtomicU64::new(0);
    let write = WRITES.fetch_add(1, Ordering::Relaxed);
    format!(".{name}.{}.{write}.tmp", std::process::id())
}

/// Writes `bytes` to `temporary`, which must not exist yet, at mode `0600`,
/// and renames it to `path`. The temporary file is removed on every way out
/// that did not rename it.
pub(super) fn write_exclusively(
    temporary: &Path,
    path: &Path,
    bytes: &[u8],
) -> Result<(), LaunchFileError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(FILE_MODE)
        .open(temporary)
        .map_err(LaunchFileError::io("creating the file"))?;
    let written = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(LaunchFileError::io("writing the file"))
        .and_then(|()| {
            fs::rename(temporary, path).map_err(LaunchFileError::io("renaming the file"))
        });
    if written.is_err() {
        // The failure being reported is the one that matters; the temporary
        // file holds a ticket nobody will be told of, which is spent within a
        // minute either way.
        let _ = fs::remove_file(temporary);
    }
    written
}

/// Refuses a directory that is not one, is not this account's, or lets anybody
/// else in.
fn check(path: &Path) -> Result<(), LaunchFileError> {
    let metadata =
        fs::symlink_metadata(path).map_err(LaunchFileError::io("inspecting the directory"))?;
    judge(
        metadata.file_type().is_dir(),
        metadata.uid(),
        metadata.mode(),
        rustix::process::geteuid().as_raw(),
    )
}

/// The verdict on a directory that is a directory or not, owned by `owner`, at
/// `mode`, as seen by the account `me`.
pub(super) fn judge(is_dir: bool, owner: u32, mode: u32, me: u32) -> Result<(), LaunchFileError> {
    if !is_dir {
        return Err(LaunchFileError::NotADirectory);
    }
    if owner != me {
        return Err(LaunchFileError::ForeignOwner);
    }
    if mode & OTHERS != 0 {
        return Err(LaunchFileError::TooOpen { mode: mode & 0o777 });
    }
    Ok(())
}
