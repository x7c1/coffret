use std::os::fd::OwnedFd;
use std::path::Path;

use rustix::fs::{Mode, OFlags};
use rustix::io::Errno;

use super::{refusal, ConfinedDir};
use crate::fetch::descent_error::DescentError;
use crate::local_operation::LocalOperation;

/// Walks from a mapped root to the folder one file belongs in, making the
/// folders that are not there yet.
///
/// The folders are made because an Entry Path's separators are the whole of what
/// a folder is (spec: EP-2): a device fetching `albums/2026/spring.jpg` into an
/// empty mapped root has to make both. Each one is made and then *opened again*
/// rather than assumed, so a name that became a symbolic link between the two
/// calls is refused by the open rather than descended through.
///
/// `relative` is the Entry Path's components below the mapping's prefix, its
/// last being the file's own name.
pub(in crate::fetch) fn descend(
    root: &Path,
    relative: &[String],
) -> Result<ConfinedDir, DescentError> {
    let (name, folders) = split(relative);

    // Made where it is not there yet, which is what a fetch into a mapped root
    // that does not exist did before. Path-based, like the open below it, for
    // the reason `open_root` gives.
    std::fs::create_dir_all(root).map_err(|cause| DescentError::Io {
        operation: LocalOperation::Creating,
        path: root.to_path_buf(),
        cause,
    })?;

    let mut directory =
        open_root(root).map_err(|cause| refusal(root, LocalOperation::Creating, cause))?;
    let mut folder = root.to_path_buf();
    for step in folders {
        folder.push(step);
        directory = enter_or_make(&directory, step, &folder)?;
    }
    Ok(ConfinedDir {
        directory,
        folder,
        name: name.clone(),
    })
}

/// The same walk over the folders that are already there, making none.
///
/// `None` where a folder on the way is not there at all: nothing can stand at
/// the file's path if the folder above it does not exist, which is the same
/// answer as an empty place rather than a refusal. A symbolic link *is* a
/// refusal, at any depth, whether it points inside the mapped root or out of it
/// — the canonical place for the Entry is the one the mappings name, and a
/// second name for it is not that place (spec: EP-9, EP-4).
pub(in crate::fetch) fn look_up(
    root: &Path,
    relative: &[String],
) -> Result<Option<ConfinedDir>, DescentError> {
    let (name, folders) = split(relative);

    let mut directory = match open_root(root) {
        Ok(directory) => directory,
        Err(absent) if absent == Errno::NOENT => return Ok(None),
        Err(cause) => return Err(refusal(root, LocalOperation::Stating, cause)),
    };
    let mut folder = root.to_path_buf();
    for step in folders {
        folder.push(step);
        directory = match enter(&directory, step) {
            Ok(entered) => entered,
            Err(absent) if absent == Errno::NOENT => return Ok(None),
            Err(cause) => return Err(refusal(&folder, LocalOperation::Stating, cause)),
        };
    }
    Ok(Some(ConfinedDir {
        directory,
        folder,
        name: name.clone(),
    }))
}

/// The file's own name and the folders above it.
///
/// A translated place always has at least one component — it is a mapping's
/// local root with the Entry Path's components below the prefix pushed onto it,
/// and an Entry standing at exactly the prefix is refused before a place is made
/// at all (spec: EP-9). So the split is an assertion rather than a question.
fn split(relative: &[String]) -> (&String, &[String]) {
    relative
        .split_last()
        .expect("a place under a mapped root names at least the file itself")
}

/// Opens the mapped root, which is the one name this walk resolves as a path.
///
/// Deliberately without `O_NOFOLLOW`: the root is the folder the person
/// configured this device to keep a subtree in, so what that path points at is
/// their choice to make (spec: EP-9). Every name *below* it comes from the
/// Library and is descended into instead.
fn open_root(root: &Path) -> Result<OwnedFd, Errno> {
    rustix::fs::open(
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
}

/// Descends one name, making the folder where it is not there yet.
fn enter_or_make(directory: &OwnedFd, name: &str, at: &Path) -> Result<OwnedFd, DescentError> {
    match enter(directory, name) {
        Ok(entered) => return Ok(entered),
        Err(absent) if absent == Errno::NOENT => {}
        Err(cause) => return Err(refusal(at, LocalOperation::Creating, cause)),
    }
    // 0o777 before the umask, which is what `create_dir_all` asks for.
    match rustix::fs::mkdirat(directory, name, Mode::from_bits_truncate(0o777)) {
        Ok(()) => {}
        // Another writer got there first, which says nothing about what it made:
        // the open below is what decides whether this is a folder to descend.
        Err(taken) if taken == Errno::EXIST => {}
        Err(cause) => return Err(refusal(at, LocalOperation::Creating, cause)),
    }
    enter(directory, name).map_err(|cause| refusal(at, LocalOperation::Creating, cause))
}

/// Opens one name below an open folder, and only where it is a real directory.
///
/// `O_DIRECTORY` refuses anything that is not one and `O_NOFOLLOW` refuses a
/// symbolic link before it is followed, so the two together are the whole of
/// what "descend one component and stay inside the mapped root" means.
fn enter(directory: &OwnedFd, name: &str) -> Result<OwnedFd, Errno> {
    rustix::fs::openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
}
