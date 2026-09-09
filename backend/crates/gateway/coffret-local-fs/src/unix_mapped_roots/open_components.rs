use std::ffi::OsStr;
use std::io;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use coffret_usecase::{LocalIoError, LocalOperation, MappedRelativeLocation};
use rustix::fs::{Mode, OFlags};
use rustix::io::Errno;

pub(super) fn open_folder(
    root: &Path,
    relative: Option<&MappedRelativeLocation>,
    operation: LocalOperation,
) -> Result<Option<OwnedFd>, LocalIoError> {
    open_components(
        root,
        relative
            .into_iter()
            .flat_map(MappedRelativeLocation::components),
        operation,
    )
}

pub(super) fn open_components<'a>(
    root: &Path,
    components: impl Iterator<Item = &'a OsStr>,
    operation: LocalOperation,
) -> Result<Option<OwnedFd>, LocalIoError> {
    let mut directory = match rustix::fs::open(
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(directory) => directory,
        Err(Errno::NOENT) => return Ok(None),
        Err(cause) => return Err(local_error(operation, root, cause)),
    };
    let mut at = root.to_path_buf();
    for component in components {
        at.push(component);
        directory = match rustix::fs::openat(
            &directory,
            component,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(directory) => directory,
            Err(Errno::NOENT) => return Ok(None),
            Err(cause) => return Err(local_error(operation, &at, cause)),
        };
    }
    Ok(Some(directory))
}

pub(super) fn local_error(
    operation: LocalOperation,
    path: impl Into<PathBuf>,
    cause: Errno,
) -> LocalIoError {
    LocalIoError::new(
        operation,
        path,
        io::Error::from_raw_os_error(cause.raw_os_error()),
    )
}
