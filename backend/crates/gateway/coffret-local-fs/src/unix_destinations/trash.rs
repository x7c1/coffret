//! Moving a file out of a mapped folder into the desktop's own trash
//! (spec: EP-15).
//!
//! The same descent a placement makes, asked for a different write: the root is
//! opened as the person named it and held against its mapping's marker from that
//! open handle (spec: EP-13), every folder below it is entered without following
//! a link (spec: EP-8), and the file's own name is stated without following one
//! and has to be a regular file. Only then is the trash asked to take it.
//!
//! The trash is asked by path, because that is the only way any platform's trash
//! can be asked: the freedesktop.org Trash records the original path in the
//! entry it writes and renames the file into the trash directory of the volume
//! it stands on, and the Finder's takes a URL. So the move resolves the path
//! once more after the descent has vouched for it, and a folder swapped for a
//! symbolic link in that interval would be followed. What that costs is a file
//! of the person's moved to their own trash rather than deleted, which is the
//! least any race here can do; it is said so that nobody reads the descent as
//! the whole of the guarantee.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::{
    DescentError, LocalIoError, LocalOperation, LocalTrash, MappedRelativeLocation,
};
use rustix::fs::{AtFlags, FileType, Mode, OFlags};

use crate::unix_destinations::{blocking, descent, refusal, vouch};
use crate::unix_fs::UnixFs;

#[async_trait]
impl LocalTrash for UnixFs {
    async fn move_to_trash(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        relative: &MappedRelativeLocation,
    ) -> Result<(), DescentError> {
        let owned = root.to_path_buf();
        let expected = expected.copied();
        let relative = relative.clone();
        blocking(LocalOperation::MovingToTrash, root, move || {
            let file = vouched(&owned, expected.as_ref(), &relative)?;
            to_trash(&file).map_err(|cause| {
                DescentError::Io(LocalIoError::new(
                    LocalOperation::MovingToTrash,
                    &file,
                    io::Error::other(cause),
                ))
            })
        })
        .await
    }
}

/// Walks to the file below a vouched root and confirms it is a regular file,
/// answering with the path the trash is then asked to take.
fn vouched(
    root: &Path,
    expected: Option<&RootMarkerId>,
    relative: &MappedRelativeLocation,
) -> Result<PathBuf, DescentError> {
    let mut directory =
        descent::open_root(root).map_err(|cause| refusal(root, LocalOperation::Stating, cause))?;
    vouch::vouch(&directory, root, expected)?;

    let components: Vec<&OsStr> = relative.components().collect();
    let (name, folders) = components
        .split_last()
        .expect("a file below a mapped root names at least itself");
    let mut at = root.to_path_buf();
    for step in folders {
        at.push(step);
        directory = rustix::fs::openat(
            &directory,
            *step,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|cause| refusal(&at, LocalOperation::Stating, cause))?;
    }

    at.push(name);
    let stat = rustix::fs::statat(&directory, *name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|cause| refusal(&at, LocalOperation::Stating, cause))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(DescentError::Io(LocalIoError::new(
            LocalOperation::MovingToTrash,
            &at,
            io::Error::other("what stands at this path is not a regular file"),
        )));
    }
    Ok(at)
}

/// Asks the Finder's trash to take one file, through the file manager rather
/// than by scripting the Finder.
///
/// The scripted way is the `trash` crate's default, and it is the wrong one for
/// a sync: it asks the person to let coffret control the Finder, and plays the
/// Finder's sound for every file a run moves. The file manager's way does
/// neither and still puts the file in the same trash.
#[cfg(target_os = "macos")]
fn to_trash(file: &Path) -> Result<(), trash::Error> {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};

    let mut context = trash::TrashContext::default();
    context.set_delete_method(DeleteMethod::NsFileManager);
    context.delete(file)
}

/// Asks the freedesktop.org Trash to take one file: the trash directory of the
/// volume it stands on, or the person's own where that is the same volume.
#[cfg(not(target_os = "macos"))]
fn to_trash(file: &Path) -> Result<(), trash::Error> {
    trash::delete(file)
}
