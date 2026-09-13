use std::fs::File;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::root_marker::{self, MANAGEMENT_AREA, MARKER_FILE};
use coffret_usecase::{DescentError, LocalIoError, LocalOperation, RootRefused};
use rustix::fs::{Mode, OFlags};
use rustix::io::Errno;

/// Holds the marker standing in an open mapped root against the identity its
/// mapping records (spec: EP-13).
///
/// `directory` is the root the descent has *just opened* and the one the
/// placement then writes through. That is the whole point of asking here: a check
/// made against a re-resolved path would answer about a folder the write need not
/// land in, which is exactly the race the rule rules out. Every name below the
/// handle is opened without following links, so a `.coffret` or a `root` swapped
/// under the walk cannot redirect the answer either (spec: EP-8).
///
/// Read-only, and deliberately so: nothing here creates, completes, or repairs
/// anything. Only recording a mapping ever writes or adopts a marker, so a
/// placement that meets a missing or broken one refuses and says which case it
/// was.
///
/// `root` is carried only to name the folder in the refusal. Nothing reaches the
/// filesystem through it.
pub(super) fn vouch(
    directory: &OwnedFd,
    root: &Path,
    expected: Option<&RootMarkerId>,
) -> Result<(), DescentError> {
    let refused = |reason| {
        Err(DescentError::Refused {
            root: root.to_path_buf(),
            reason,
        })
    };

    // Asked before the folder is looked at, because a mapping with no identity
    // has nothing to hold a marker against however sound the marker is.
    let Some(expected) = expected else {
        return refused(RootRefused::NoExpectedIdentity);
    };

    let area = match enter(directory, MANAGEMENT_AREA) {
        Ok(area) => area,
        Err(Errno::NOENT) => return refused(RootRefused::ManagementAreaMissing),
        // `ELOOP` and `ENOTDIR` both arrive here, and the descent's own
        // `refusal` beside this makes one reading of the pair: a symbolic link
        // comes as one or the other depending on the platform, since this open
        // passes `O_DIRECTORY` beside `O_NOFOLLOW`, and anything else that is
        // not a folder comes as `ENOTDIR`. The rule makes one case of all of
        // them: the name is reserved for coffret's own folder and something else
        // is standing at it (spec: EP-13, EP-14).
        Err(Errno::LOOP | Errno::NOTDIR) => {
            return refused(RootRefused::ManagementAreaNotADirectory)
        }
        Err(cause) => {
            return Err(refused_by_the_system(
                area_path(root),
                LocalOperation::Stating,
                std::io::Error::from(cause),
            ))
        }
    };

    let file = match rustix::fs::openat(
        &area,
        MARKER_FILE,
        // `O_NONBLOCK` so that a named pipe standing at the name answers the
        // open instead of holding it: what it is gets decided below, and an open
        // that never returns decides nothing.
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(file) => File::from(file),
        Err(Errno::NOENT) => return refused(RootRefused::MarkerMissing),
        // The link the open turned away: the identity read through one would be
        // whatever it points at rather than this root's. `ELOOP` alone here,
        // where the area's open above reads two, because this open passes no
        // `O_DIRECTORY` for a platform to answer `ENOTDIR` to.
        Err(Errno::LOOP) => return refused(RootRefused::MarkerNotARegularFile),
        Err(cause) => {
            return Err(refused_by_the_system(
                marker_path(root),
                LocalOperation::Reading,
                std::io::Error::from(cause),
            ))
        }
    };

    // `O_NOFOLLOW` turned a symbolic link away and nothing else: a folder, a
    // device, a socket, and a pipe all open. What the handle is is asked of the
    // handle, so a name swapped after the open cannot change the answer.
    let regular = match file.metadata() {
        Ok(stated) => stated.is_file(),
        Err(cause) => {
            return Err(refused_by_the_system(
                marker_path(root),
                LocalOperation::Stating,
                cause,
            ))
        }
    };
    if !regular {
        return refused(RootRefused::MarkerNotARegularFile);
    }

    // One byte past the cap, which is the least that shows a file running on
    // past it: the rest of such a file is never read, so a root pointed at an
    // enormous or an endless one costs this read the cap and a byte and no more
    // (spec: EP-13).
    let mut content = Vec::with_capacity(root_marker::MAX_LEN + 1);
    if let Err(cause) = file
        .take((root_marker::MAX_LEN + 1) as u64)
        .read_to_end(&mut content)
    {
        return Err(refused_by_the_system(
            marker_path(root),
            LocalOperation::Reading,
            cause,
        ));
    }

    match root_marker::parse(&content) {
        Err(cause) => refused(RootRefused::MarkerMalformed { cause }),
        Ok(found) if found != *expected => refused(RootRefused::MarkerMismatch),
        Ok(_) => Ok(()),
    }
}

/// Descends one name below an open folder, and only where it is a real
/// directory.
fn enter(directory: &OwnedFd, name: &str) -> Result<OwnedFd, Errno> {
    rustix::fs::openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
}

/// What the operating system refused, for a reason that is none of EP-13's
/// cases.
///
/// An I/O refusal rather than a verdict about the root's identity: a permission
/// the process does not have says nothing about which folder this is, and
/// reading it as a mismatch would send a person to record the mapping again over
/// something that is not about the mapping at all.
fn refused_by_the_system(
    path: PathBuf,
    operation: LocalOperation,
    cause: std::io::Error,
) -> DescentError {
    DescentError::Io(LocalIoError::new(operation, path, cause))
}

/// The management area's path, for a refusal that names the folder it is about.
fn area_path(root: &Path) -> PathBuf {
    root.join(MANAGEMENT_AREA)
}

/// The marker's own path, for the same.
fn marker_path(root: &Path) -> PathBuf {
    area_path(root).join(MARKER_FILE)
}
