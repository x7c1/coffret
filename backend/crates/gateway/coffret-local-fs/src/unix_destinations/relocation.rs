//! Relocating a file inside a mapped folder to follow its Entry (spec: EP-10).
//!
//! The same descent a placement makes, asked for a rename: the root is opened
//! as the person named it and held against its mapping's marker from that open
//! handle (spec: EP-13), every folder on the way to either name is entered
//! without following a link (spec: EP-8), and the file's own name is stated
//! without following one and has to be a regular file. The rename itself is
//! made between the two open folders, so nothing is resolved by path again.
//!
//! One rename and nothing in its place. `RENAME_NOREPLACE` (`RENAME_EXCL` on
//! macOS) is what keeps a file that arrived at the destination from being
//! overwritten, and `RENAME_EXCHANGE` (`RENAME_SWAP`) is what lets two files
//! take each other's names with neither ever replaced. A filesystem that offers
//! neither flag — Linux answers `EINVAL`, macOS `ENOTSUP`, as FAT volumes and
//! network shares can — refuses the relocation rather than asking whether the
//! name is free and renaming onto it, a window a file arriving in it would
//! lose: the file waits where it stands, and the refusal names the filesystem
//! as the reason. A rename across two filesystems is refused, and never made
//! by copying and deleting.

use std::ffi::OsStr;
use std::io;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::{
    DescentError, LocalIoError, LocalOperation, MappedRelativeLocation, RelocateError, Relocations,
};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, RenameFlags};
use rustix::io::Errno;

use crate::unix_destinations::{blocking, descent, refusal, vouch};
use crate::unix_fs::UnixFs;

#[async_trait]
impl Relocations for UnixFs {
    async fn relocate(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        from: &MappedRelativeLocation,
        to: &MappedRelativeLocation,
    ) -> Result<(), RelocateError> {
        let owned = root.to_path_buf();
        let expected = expected.copied();
        let from = from.clone();
        let to = to.clone();
        blocking(LocalOperation::Relocating, root, move || {
            relocate(&owned, expected.as_ref(), &from, &to)
        })
        .await
    }

    async fn exchange(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        one: &MappedRelativeLocation,
        other: &MappedRelativeLocation,
    ) -> Result<(), RelocateError> {
        let owned = root.to_path_buf();
        let expected = expected.copied();
        let one = one.clone();
        let other = other.clone();
        blocking(LocalOperation::Relocating, root, move || {
            exchange(&owned, expected.as_ref(), &one, &other)
        })
        .await
    }
}

/// Renames the file at `from` to `to`, making the folders `to` needs and
/// removing them again where the rename is refused.
fn relocate(
    root: &Path,
    expected: Option<&RootMarkerId>,
    from: &MappedRelativeLocation,
    to: &MappedRelativeLocation,
) -> Result<(), RelocateError> {
    let directory = vouched_root(root, expected)?;
    let source = regular_file(&directory, root, from)?;
    let (target, made) = folder_for(&directory, root, to)?;

    let renamed = rustix::fs::renameat_with(
        &source.folder,
        &source.name,
        &target.folder,
        &target.name,
        RenameFlags::NOREPLACE,
    );
    match renamed {
        Ok(()) => Ok(()),
        Err(cause) => {
            unmake(made);
            Err(match cause {
                Errno::EXIST | Errno::NOTEMPTY => RelocateError::Taken { at: target.path },
                Errno::XDEV => RelocateError::AnotherFilesystem {
                    from: source.path,
                    to: target.path,
                },
                Errno::INVAL | Errno::NOTSUP => unoffered(target.path, cause),
                cause => {
                    DescentError::from(refusal(&source.path, LocalOperation::Relocating, cause))
                        .into()
                }
            })
        }
    }
}

/// Exchanges the files at `one` and `other` with one rename.
fn exchange(
    root: &Path,
    expected: Option<&RootMarkerId>,
    one: &MappedRelativeLocation,
    other: &MappedRelativeLocation,
) -> Result<(), RelocateError> {
    let directory = vouched_root(root, expected)?;
    let first = regular_file(&directory, root, one)?;
    let second = regular_file(&directory, root, other)?;
    match rustix::fs::renameat_with(
        &first.folder,
        &first.name,
        &second.folder,
        &second.name,
        RenameFlags::EXCHANGE,
    ) {
        Ok(()) => Ok(()),
        Err(Errno::XDEV) => Err(RelocateError::AnotherFilesystem {
            from: first.path,
            to: second.path,
        }),
        Err(cause @ (Errno::INVAL | Errno::NOTSUP)) => Err(unoffered(first.path, cause)),
        Err(cause) => {
            Err(DescentError::from(refusal(&first.path, LocalOperation::Relocating, cause)).into())
        }
    }
}

/// The refusal for a filesystem that does not offer the rename asked of it
/// (`EINVAL` on Linux, `ENOTSUP` on macOS): nothing weaker is made in its place,
/// and the answer the operating system gave travels with it.
fn unoffered(at: PathBuf, cause: Errno) -> RelocateError {
    RelocateError::NoAtomicRename {
        at,
        cause: cause.into(),
    }
}

/// One name inside a folder held open.
struct Named {
    folder: OwnedFd,
    name: PathBuf,
    /// Where it stands, for a refusal to name — never for reaching it again.
    path: PathBuf,
}

/// Opens the mapped root and holds it against its mapping's marker.
fn vouched_root(root: &Path, expected: Option<&RootMarkerId>) -> Result<OwnedFd, DescentError> {
    let directory = descent::open_root(root)
        .map_err(|cause| DescentError::from(refusal(root, LocalOperation::Stating, cause)))?;
    vouch::vouch(&directory, root, expected)?;
    Ok(directory)
}

/// Walks to the folder holding the file at `relative`, and states the file
/// without following a link: it has to be a regular file.
fn regular_file(
    root_folder: &OwnedFd,
    root: &Path,
    relative: &MappedRelativeLocation,
) -> Result<Named, DescentError> {
    let components: Vec<&OsStr> = relative.components().collect();
    let (name, folders) = components
        .split_last()
        .expect("a file below a mapped root names at least itself");
    let mut directory = root_folder
        .try_clone()
        .map_err(|cause| LocalIoError::new(LocalOperation::Stating, root, cause))?;
    let mut at = root.to_path_buf();
    for step in folders {
        at.push(step);
        directory = enter(&directory, step)
            .map_err(|cause| DescentError::from(refusal(&at, LocalOperation::Stating, cause)))?;
    }
    at.push(name);
    let stat = rustix::fs::statat(&directory, *name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|cause| DescentError::from(refusal(&at, LocalOperation::Stating, cause)))?;
    match FileType::from_raw_mode(stat.st_mode) {
        FileType::RegularFile => Ok(Named {
            folder: directory,
            name: PathBuf::from(name),
            path: at,
        }),
        // Not followed: what stands past a link is not the file this device
        // placed (spec: EP-8).
        FileType::Symlink => Err(DescentError::Blocked { stopped_at: at }),
        _ => Err(DescentError::Io(LocalIoError::new(
            LocalOperation::Relocating,
            &at,
            io::Error::other("what stands at this path is not a regular file"),
        ))),
    }
}

/// Walks to the folder `relative` belongs in, making the folders that are not
/// there yet, and answers with the ones it made.
fn folder_for(
    root_folder: &OwnedFd,
    root: &Path,
    relative: &MappedRelativeLocation,
) -> Result<(Named, Vec<(OwnedFd, PathBuf)>), DescentError> {
    let components: Vec<&OsStr> = relative.components().collect();
    let (name, folders) = components
        .split_last()
        .expect("a file below a mapped root names at least itself");
    let mut directory = root_folder
        .try_clone()
        .map_err(|cause| LocalIoError::new(LocalOperation::Creating, root, cause))?;
    let mut at = root.to_path_buf();
    let mut made = Vec::new();
    for step in folders {
        at.push(step);
        let entered = match enter(&directory, step) {
            Ok(entered) => Ok(entered),
            Err(Errno::NOENT) => make(&directory, step).and_then(|()| {
                let parent = directory
                    .try_clone()
                    .map_err(|cause| Errno::from_io_error(&cause).unwrap_or(Errno::IO))?;
                made.push((parent, PathBuf::from(step)));
                enter(&directory, step)
            }),
            Err(cause) => Err(cause),
        };
        directory = match entered {
            Ok(entered) => entered,
            Err(cause) => {
                unmake(made);
                return Err(refusal(&at, LocalOperation::Creating, cause).into());
            }
        };
    }
    at.push(name);
    Ok((
        Named {
            folder: directory,
            name: PathBuf::from(name),
            path: at,
        },
        made,
    ))
}

/// Makes one folder, another writer having made it first being as good.
fn make(directory: &OwnedFd, name: &OsStr) -> Result<(), Errno> {
    match rustix::fs::mkdirat(directory, name, Mode::from_bits_truncate(0o777)) {
        Ok(()) | Err(Errno::EXIST) => Ok(()),
        Err(cause) => Err(cause),
    }
}

/// Removes the folders a refused relocation made, the deepest first, each only where
/// nothing is in it; a refusal here leaves an empty folder and nothing worse.
fn unmake(made: Vec<(OwnedFd, PathBuf)>) {
    for (parent, name) in made.into_iter().rev() {
        let _ = rustix::fs::unlinkat(&parent, &name, AtFlags::REMOVEDIR);
    }
}

/// Opens one name below an open folder, and only where it is a real directory.
fn enter(directory: &OwnedFd, name: &OsStr) -> Result<OwnedFd, Errno> {
    rustix::fs::openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
}
