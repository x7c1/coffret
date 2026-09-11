use std::os::fd::OwnedFd;
use std::path::Path;

use coffret_usecase::root_marker::MANAGEMENT_AREA;
use coffret_usecase::LocalOperation;
use rustix::fs::{Mode, OFlags};
use rustix::io::Errno;

use super::area_path;
use crate::error::{Error, Result};

/// The management area below an open root, made where it was not there.
pub(super) enum ManagementArea {
    /// It was not there and this call made it, so the marker is this call's to
    /// write.
    Made(OwnedFd),
    /// It was already there, so whatever stands at the marker's name is
    /// somebody's and is read rather than written over.
    Found(OwnedFd),
}

/// Opens the mapped root, which is the one name this descent resolves as a path.
///
/// Deliberately without `O_NOFOLLOW`: the root is the folder the person
/// configured this device to keep a subtree in, so what that path points at is
/// their choice to make (spec: EP-8, EP-9). Every name below it is descended
/// into instead.
pub(super) fn open_root(root: &Path) -> Result<OwnedFd> {
    rustix::fs::open(
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|cause| match cause {
        // The root was canonicalized and found to be a directory a moment ago,
        // so this is a folder that went away in between rather than a typo —
        // which is the same thing to say about it either way.
        Errno::NOENT | Errno::NOTDIR => Error::NoSuchLocalRoot {
            path: root.to_path_buf(),
            cause: Some(cause.into()),
        },
        cause => Error::local(LocalOperation::Stating, root)(cause.into()),
    })
}

/// Enters `.coffret` below the root, making it where it is absent.
///
/// A symbolic link and anything that is not a folder are one refusal here:
/// `O_NOFOLLOW` reports `ELOOP` for the first — `EMLINK` where the BSDs spell it
/// that way — and `O_DIRECTORY` reports `ENOTDIR` for the second, and what the
/// message has to say is the same for both — the name is reserved for coffret's
/// own folder and something else is standing at it (spec: EP-13, EP-14). Both
/// spellings of the link are read here and after the racing `mkdirat` below,
/// because the placement side reads both and a registration that read one would
/// report a local I/O failure where the rule names a verdict.
pub(super) fn enter_or_make(directory: &OwnedFd, root: &Path) -> Result<ManagementArea> {
    match enter(directory, MANAGEMENT_AREA) {
        Ok(area) => return Ok(ManagementArea::Found(area)),
        Err(Errno::NOENT) => {}
        Err(Errno::LOOP | Errno::MLINK | Errno::NOTDIR) => {
            return Err(Error::ManagementAreaNotADirectory {
                root: root.to_path_buf(),
            })
        }
        Err(cause) => {
            return Err(Error::local(LocalOperation::Stating, area_path(root))(
                cause.into(),
            ))
        }
    }

    // 0o777 before the umask, which is what `create_dir` asks for.
    match rustix::fs::mkdirat(directory, MANAGEMENT_AREA, Mode::from_bits_truncate(0o777)) {
        Ok(()) => enter(directory, MANAGEMENT_AREA)
            .map(ManagementArea::Made)
            .map_err(|cause| Error::local(LocalOperation::Creating, area_path(root))(cause.into())),
        // Another registration got there first between the open and the mkdir,
        // which says nothing about what it made: the entry below is what decides
        // whether this is a folder at all, and it is that run's marker rather
        // than this one's that stands in it.
        Err(Errno::EXIST) => match enter(directory, MANAGEMENT_AREA) {
            Ok(area) => Ok(ManagementArea::Found(area)),
            Err(Errno::LOOP | Errno::MLINK | Errno::NOTDIR) => {
                Err(Error::ManagementAreaNotADirectory {
                    root: root.to_path_buf(),
                })
            }
            Err(cause) => Err(Error::local(LocalOperation::Stating, area_path(root))(
                cause.into(),
            )),
        },
        Err(cause) => Err(Error::local(LocalOperation::Creating, area_path(root))(
            cause.into(),
        )),
    }
}

/// Descends one name below an open folder, and only where it is a real
/// directory.
fn enter(directory: &OwnedFd, name: &str) -> std::result::Result<OwnedFd, Errno> {
    rustix::fs::openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
}
