use std::os::fd::OwnedFd;
use std::path::Path;

use coffret_usecase::root_marker::{self, MANAGEMENT_AREA};
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
/// `ELOOP` and `ENOTDIR` say the same thing about the name, which is that it is
/// reserved for coffret's own folder and something else is standing at it
/// (spec: EP-13, EP-14). Which of the two a link arrives as depends on the
/// platform, since this open passes `O_DIRECTORY` beside `O_NOFOLLOW`. Both are
/// read here and again after the racing `mkdirat` below, because the placement
/// side reads the same pair and a registration that read only one of them would
/// report a local I/O failure where the rule names a verdict.
pub(super) fn enter_or_make(directory: &OwnedFd, root: &Path) -> Result<ManagementArea> {
    match enter(directory, MANAGEMENT_AREA) {
        Ok(area) => return Ok(ManagementArea::Found(area)),
        Err(Errno::NOENT) => {}
        Err(Errno::LOOP | Errno::NOTDIR) => {
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
            Err(Errno::LOOP | Errno::NOTDIR) => Err(Error::ManagementAreaNotADirectory {
                root: root.to_path_buf(),
            }),
            Err(cause) => Err(Error::local(LocalOperation::Stating, area_path(root))(
                cause.into(),
            )),
        },
        Err(cause) => Err(Error::local(LocalOperation::Creating, area_path(root))(
            cause.into(),
        )),
    }
}

/// The refusal for a management area that holds no marker, with the name the
/// descent actually reached read back off `root` (spec: EP-13, EP-14).
///
/// [`enter`] opens by name and hands back a file descriptor, and a descriptor
/// carries no name: on a filesystem that folds ASCII case, an open of
/// `.coffret` reaches an on-disk `.COFFRET` and nothing downstream of it can
/// tell which of the two it got. So the spelling is read here, where the
/// refusal is composed, and nowhere else — a directory read on a path that is
/// already failing costs nothing anybody notices, and a registration that is
/// succeeding never does it at all.
///
/// By the path, and so by resolving the root a second time, which is what
/// EP-13 otherwise rules out. That rule is about *placement*: a re-resolved
/// path may be a different folder from the one the run vouched for, and writing
/// into it would put something where the mapping never pointed. Nothing is
/// written or adopted here — the registration has already failed, and this only
/// settles which sentence it fails with. A folder swapped in between can
/// therefore cost one thing and no more: a refusal naming a name that was not
/// the one the descent reached. It still refuses, and it still writes nothing.
///
/// The exact spelling wins wherever it is there: on a volume that folds, only
/// one of the two names can be on disk anyway; on one that does not, `.coffret`
/// standing beside `.COFFRET` means the descent reached `.coffret` and the
/// interrupted-registration sentence is the true one. A read the operating
/// system refuses, or a listing with neither name in it — the folder renamed in
/// between — leaves that same sentence standing, because nothing was learned to
/// say anything else with.
pub(super) fn missing_marker(root: &Path) -> Error {
    let interrupted = Error::ManagementAreaIncomplete {
        root: root.to_path_buf(),
    };
    let Ok(entries) = std::fs::read_dir(root) else {
        return interrupted;
    };
    let mut folded = None;
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if root_marker::is_management_area(&name) {
            return interrupted;
        }
        if folded.is_none() && root_marker::folds_to_management_area(&name) {
            folded = Some(name);
        }
    }
    match folded {
        Some(name) => Error::ManagementAreaFolded {
            root: root.to_path_buf(),
            name,
        },
        None => interrupted,
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

#[cfg(test)]
mod tests {
    use coffret_model::Redacted;

    use super::*;

    /// A root with the given names standing in it and nothing else.
    ///
    /// A name already there is left alone rather than asserted on, because the
    /// suite itself may be running on a volume that folds: two of these names
    /// are one folder there, which is the very state under test and not a
    /// failure to set one up.
    fn root_holding(names: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
        let held = tempfile::tempdir().expect("a temporary directory must be available");
        let root = held.path().join("library");
        std::fs::create_dir(&root).expect("the folder must be creatable");
        for name in names {
            match std::fs::create_dir(root.join(name)) {
                Ok(()) => {}
                Err(cause) if cause.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(cause) => panic!("the folder must be creatable: {cause}"),
            }
        }
        (held, root)
    }

    // EP-14: the sentence about an interrupted registration is a sentence about
    // coffret's own folder, and on a case-folding volume the descent may have
    // landed in one of the person's. The fold is driven through the comparison
    // rather than through a real folding filesystem — what a folding kernel does
    // is the one thing only hardware can show — so this stands the name up on
    // disk and asks what the refusal is composed of.
    #[test]
    fn a_folder_that_folds_to_the_reserved_name_is_not_called_an_interrupted_registration() {
        let (_held, root) = root_holding(&[".COFFRET"]);

        let refused = missing_marker(&root);
        let said = refused.to_string();
        assert!(
            matches!(&refused, Error::ManagementAreaFolded { name, .. } if name == ".COFFRET"),
            "the refusal must name the folder standing there, and was {refused:?}"
        );
        assert!(
            said.contains(".COFFRET"),
            "a person is told which folder it is about: {said}"
        );
        assert!(
            !said.contains("interrupted"),
            "and is never told an interrupted registration left their folder: {said}"
        );
        // The root and the name in it are both the person's own, so neither
        // reaches a diagnostic event (spec: EL-1).
        assert_eq!(refused.redacted(), "Device::ManagementAreaFolded");
    }

    // And the exact name keeps the sentence it has: this is the state EP-13
    // names, and nothing about it changed.
    #[test]
    fn the_reserved_name_itself_is_still_an_interrupted_registration() {
        for standing in [&[MANAGEMENT_AREA][..], &[MANAGEMENT_AREA, ".COFFRET"][..]] {
            let (_held, root) = root_holding(standing);

            let refused = missing_marker(&root);
            assert!(
                matches!(&refused, Error::ManagementAreaIncomplete { .. }),
                "with {standing:?} standing there the descent reached {MANAGEMENT_AREA}, \
                 and was {refused:?}"
            );
            assert!(refused.to_string().contains("interrupted registration"));
        }
    }

    // Nothing of either name: the folder was renamed between the descent and
    // the listing, and nothing was learned to say anything new with.
    #[test]
    fn a_root_that_no_longer_holds_either_name_keeps_the_older_sentence() {
        let (_held, root) = root_holding(&["albums"]);

        assert!(matches!(
            missing_marker(&root),
            Error::ManagementAreaIncomplete { .. }
        ));
    }
}
