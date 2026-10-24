use std::fmt;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use crate::descent_error::DescentError;
use crate::unavailable_root::RootUnavailable;

/// Why a file waiting to follow its Entry was not relocated this run
/// (spec: EP-10).
///
/// Every one of them leaves the file exactly where it stands and its record as
/// it was, still waiting: nothing is overwritten, copied or uploaded, and the
/// next run that relocates asks again. So each is a finding a person may act on
/// — plug the disk in, clear the file that is in the way — and none is an error.
///
/// There is deliberately no `PartialEq`: two variants carry a refusal from the
/// disk, which is reported rather than compared.
#[derive(Debug, Clone)]
pub enum RelocationRefused {
    /// The mapped root the file stands under is not there to be read from or
    /// written to (spec: EP-12).
    Unavailable(RootUnavailable),
    /// The file and its Entry's new path do not lie under one mapped root: two
    /// different mappings, or a path no mapping of this device reaches. A
    /// rename does not cross from one root to another (spec: EP-9).
    OutsideOneRoot,
    /// The new path passes through a name a mapped folder keeps for something
    /// other than content — the device's own management area, a fetch's
    /// scratch, a desktop's trash folder at the root's top, or a name an
    /// operating system keeps — and no file is placed there (spec: EP-11,
    /// EP-14, EP-16, EP-17).
    ReservedName,
    /// The file and where it is to go stand on two filesystems, and nothing is
    /// copied across in place of a rename.
    AnotherFilesystem,
    /// A name on the way to the file or to where it is to go is not a folder of
    /// the mapped root — a symbolic link among them — and nothing is relocated
    /// through it (spec: EP-8).
    SymbolicLink {
        /// The local path of that name.
        stopped_at: PathBuf,
    },
    /// The file changed since this device placed it, so it is not the copy the
    /// record is about: it is kept where it stands (spec: EP-15).
    Edited,
    /// Something this device did not place for the Entry already stands where
    /// the file is to go, and it is never overwritten (spec: EP-11).
    DestinationTaken,
    /// This device's own file of another Entry stands where the file is to go,
    /// itself waiting to be relocated, and it was not relocated out of the way
    /// this run — refused for a reason of its own, or left where it stood by
    /// an exchange of a cycle that was refused partway (spec: EP-10).
    ///
    /// Not a file of the person's: once the one in the way is relocated, this
    /// one follows.
    WaitingFileInTheWay,
    /// The filesystem the file stands on offers no rename that never replaces
    /// what stands at the new path — as FAT volumes and network shares can —
    /// and nothing weaker is made in its place (spec: EP-10, EP-11).
    ///
    /// Unlike the disk's other refusals, one the same filesystem gives every
    /// run: the file keeps waiting until the folder is on one that offers it.
    NoAtomicRename(Arc<io::Error>),
    /// The root would not vouch for itself, or the disk or the trash refused
    /// (spec: EP-13, EP-15).
    Disk(Arc<DescentError>),
}

impl fmt::Display for RelocationRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(RootUnavailable::Missing) => {
                f.write_str("the mapped folder it stands in is not there")
            }
            Self::Unavailable(RootUnavailable::AnotherFilesystem) => f.write_str(
                "the mapped folder it stands in is not on the filesystem this device recorded",
            ),
            Self::OutsideOneRoot => {
                f.write_str("its new path is not under the mapped folder the file stands in")
            }
            Self::ReservedName => f.write_str(
                "its new path passes through a name coffret, the desktop or the operating system keeps for itself",
            ),
            Self::AnotherFilesystem => {
                f.write_str("its new path is on another filesystem than the file")
            }
            Self::SymbolicLink { stopped_at } => write!(
                f,
                "a name on the way is not a folder of the mapped folder — {}",
                stopped_at.display()
            ),
            Self::Edited => f.write_str("the file changed since this device placed it"),
            Self::DestinationTaken => {
                f.write_str("a file this device did not place there stands at its new path")
            }
            Self::WaitingFileInTheWay => f.write_str(
                "this device's own file at its new path is waiting to be relocated itself and \
                 was not",
            ),
            Self::NoAtomicRename(_) => f.write_str(
                "the filesystem it stands on offers no rename that leaves what stands at its new \
                 path untouched",
            ),
            Self::Disk(_) => f.write_str("the disk refused"),
        }
    }
}

impl std::error::Error for RelocationRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Disk(cause) => Some(cause.as_ref()),
            Self::NoAtomicRename(cause) => Some(cause.as_ref()),
            _ => None,
        }
    }
}
