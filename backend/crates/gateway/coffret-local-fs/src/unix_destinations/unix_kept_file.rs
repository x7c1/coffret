use std::os::fd::OwnedFd;
use std::sync::Arc;

use coffret_usecase::{BelowRootError, KeptFile, LocalOperation};
use rustix::fs::AtFlags;
use rustix::io::Errno;

use crate::unix_destinations::open_folder::OpenFolder;

/// A file on this device's disk kept aside under a scratch name, by a second
/// link to it in the folder it stood in, for the add about to write over it
/// (spec: LA-36).
///
/// It holds the folder open behind the same [`Arc`] the destination that made
/// it holds, so putting it back and letting it go are made against that folder
/// by name, as every call of a placement is (spec: EP-4, EP-11).
pub(crate) struct UnixKeptFile {
    folder: Arc<OpenFolder>,
    /// The scratch name it is kept under.
    kept_name: String,
    /// The shared lock held on a regular file for as long as it is kept, so a
    /// sync in another process leaves it alone (spec: OC-8); `None` for a
    /// symbolic link kept aside, which no sync removes.
    _lock: Option<OwnedFd>,
}

impl UnixKeptFile {
    /// The file kept under `kept_name` in `folder`, locked by `lock`.
    pub(super) fn new(folder: Arc<OpenFolder>, kept_name: String, lock: Option<OwnedFd>) -> Self {
        Self {
            folder,
            kept_name,
            _lock: lock,
        }
    }
}

impl KeptFile for UnixKeptFile {
    fn put_back(&self) -> Result<(), BelowRootError> {
        // Within one folder, so the rename is atomic: the name carries the
        // file that replaced it or the one put back, never neither (spec: EP-11).
        rustix::fs::renameat(
            self.folder.directory(),
            self.kept_name.as_str(),
            self.folder.directory(),
            self.folder.name(),
        )
        .map_err(|cause| {
            self.folder
                .refused(self.folder.name(), LocalOperation::Renaming, cause)
        })
    }

    fn remove(&self) -> Result<(), BelowRootError> {
        match rustix::fs::unlinkat(
            self.folder.directory(),
            self.kept_name.as_str(),
            AtFlags::empty(),
        ) {
            Ok(()) => Ok(()),
            // Put back already, or removed by somebody: absence is the outcome
            // this wanted (spec: OC-8).
            Err(gone) if gone == Errno::NOENT => Ok(()),
            Err(cause) => {
                Err(self
                    .folder
                    .refused(&self.kept_name, LocalOperation::Removing, cause))
            }
        }
    }
}
