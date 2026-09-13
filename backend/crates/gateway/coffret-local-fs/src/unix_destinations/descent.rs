use std::os::fd::OwnedFd;
use std::path::Path;

use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::{DescentError, LocalOperation};
use rustix::fs::{Mode, OFlags};
use rustix::io::Errno;

use crate::unix_destinations::open_folder::OpenFolder;
use crate::unix_destinations::{refusal, vouch};

/// Walks from a mapped root to the folder one file belongs in, making the
/// folders that are not there yet.
///
/// The root is opened first and its marker is held against `expected` from that
/// open handle, before a single name below it is touched: the folder a placement
/// writes into has to be the folder the mapping was recorded against, and asking
/// through the very handle the write then uses is what keeps the question and
/// the answer about one folder (spec: EP-13). The root itself is *not* made — a
/// folder no registration ever visited carries no marker, so a local writer's
/// descent into one refuses rather than creating a folder nobody recorded.
///
/// The folders *below* it are made because an Entry Path's separators are the
/// whole of what a folder is (spec: EP-2): a local writer placing
/// `albums/2026/spring.jpg` into an empty mapped root has to make both. Each one
/// is made and then *opened again* rather than assumed, so a name that became a
/// symbolic link between the two calls is refused by the open rather than
/// descended through.
///
/// `components` is the Entry Path's components below the mapping's prefix, its
/// last being the file's own name.
pub(super) fn descend(
    root: &Path,
    expected: Option<&RootMarkerId>,
    components: &[String],
) -> Result<OpenFolder, DescentError> {
    let (name, folders) = split(components);

    // Stated rather than created: the root is opened and never made, so a
    // refusal here — a root that is not there among them — must not tell a person
    // coffret failed to create their folder. The same reading `look_up` makes of
    // the same call.
    let mut directory =
        open_root(root).map_err(|cause| refusal(root, LocalOperation::Stating, cause))?;
    vouch::vouch(&directory, root, expected)?;
    let mut folder = root.to_path_buf();
    for step in folders {
        folder.push(step);
        directory = enter_or_make(&directory, step, &folder)?;
    }
    Ok(OpenFolder::new(directory, folder, name.clone()))
}

/// The same walk over the folders that are already there, making none.
///
/// `None` where a folder on the way is not there at all: nothing can stand at
/// the file's path if the folder above it does not exist, which is the same
/// answer as an empty place rather than a refusal. A symbolic link *is* a
/// refusal, at any depth, whether it points inside the mapped root or out of it
/// — the canonical place for the Entry is the one the mappings name, and a
/// second name for it is not that place (spec: EP-9, EP-4).
pub(super) fn look_up(
    root: &Path,
    components: &[String],
) -> Result<Option<OpenFolder>, DescentError> {
    let (name, folders) = split(components);

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
    Ok(Some(OpenFolder::new(directory, folder, name.clone())))
}

/// The file's own name and the folders above it.
///
/// A translated place always has at least one component — it is a mapping's
/// local root with the Entry Path's components below the prefix pushed onto it,
/// and an Entry standing at exactly the prefix is refused before a place is made
/// at all (spec: EP-9). So the split is an assertion rather than a question.
fn split(components: &[String]) -> (&String, &[String]) {
    components
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
