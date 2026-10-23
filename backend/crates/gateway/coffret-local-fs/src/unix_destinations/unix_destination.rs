use std::path::PathBuf;
use std::sync::Arc;

use coffret_model::Redacted;
use coffret_usecase::{BelowRootError, Destination, LocalOperation, ScratchFile};
use rustix::fs::{AtFlags, FlockOperation, Mode, OFlags};
use rustix::io::Errno;
use tracing::warn;

use crate::unix_destinations::open_folder::OpenFolder;
use crate::unix_destinations::unix_scratch_file::UnixScratchFile;

/// One destination folder on this device's disk, held open from the descent
/// until the rename.
///
/// It holds the folder behind an [`Arc`] rather than owning it, because the
/// scratch it opens and the flushed file that renames that scratch are
/// two more handles on the same open folder — and each of the three may outlive
/// the others in a run that failed part way (spec: EP-11).
pub(crate) struct UnixDestination {
    folder: Arc<OpenFolder>,
}

impl UnixDestination {
    /// The destination the descent behind `folder` arrived at.
    pub(super) fn new(folder: Arc<OpenFolder>) -> Self {
        Self { folder }
    }
}

impl Destination for UnixDestination {
    fn create(&self, scratch_name: &str) -> Result<Box<dyn ScratchFile>, BelowRootError> {
        // 0o666 before the umask, which is what creating a file ordinarily asks
        // for: the file becomes the person's own on the rename, and a local
        // writer does not decide the permissions of a person's own folder.
        //
        // `O_EXCL`, so a name that already exists is a refusal rather than a
        // file two writers share, and `O_NOFOLLOW`, so a symbolic link that took
        // the name first is refused instead of followed. Callers give it a
        // scratch name, which nothing else in the folder is using.
        let opened = rustix::fs::openat(
            self.folder.directory(),
            scratch_name,
            OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_bits_truncate(0o666),
        )
        .map_err(|cause| {
            self.folder
                .refused(scratch_name, LocalOperation::Creating, cause)
        })?;
        // Locked for as long as the scratch is written, flushed and renamed —
        // the handle travels on to the rename — so a sync in another process
        // that meets it leaves it alone, while one left by a writer that is gone
        // holds no lock whatever ended it (spec: OC-8). A scratch that cannot
        // be locked is not written: on a filesystem that gives no file lock
        // (a network share, mostly) a sync could remove it mid-write, so the
        // mapped folder is refused rather than written without one. The file
        // was made just now and exclusively, so a lock somebody already holds
        // is a sync removing it in the instant since, and that is refused the
        // same way.
        //
        // Either way the empty file just made is this writer's own and nothing
        // would ever claim it, so it goes before the refusal is answered. The
        // name is unlinked rather than the handle, and that is safe because a
        // scratch name is drawn unique and never handed out twice: what stands
        // at it now is this file, or nothing where a sync removed it already.
        if let Err(cause) = rustix::fs::flock(&opened, FlockOperation::NonBlockingLockExclusive) {
            drop(opened);
            match rustix::fs::unlinkat(self.folder.directory(), scratch_name, AtFlags::empty()) {
                Ok(()) => {}
                // A sync that got to it first has removed it already.
                Err(gone) if gone == Errno::NOENT => {}
                Err(unlinked) => warn!(
                    reason = %self
                        .folder
                        .refused(scratch_name, LocalOperation::Removing, unlinked)
                        .redacted(),
                    "a scratch whose lock failed could not be removed; a later sync removes it",
                ),
            }
            return Err(self.folder.refused_here(LocalOperation::Locking, cause));
        }

        Ok(Box::new(UnixScratchFile::new(
            Arc::clone(&self.folder),
            scratch_name.to_owned(),
            tokio::fs::File::from_std(std::fs::File::from(opened)),
        )))
    }

    fn remove(&self, name: &str) -> Result<(), BelowRootError> {
        match rustix::fs::unlinkat(self.folder.directory(), name, AtFlags::empty()) {
            Ok(()) => Ok(()),
            // One that is already gone is the outcome this wanted, so a cleanup
            // racing the failure it is cleaning up after still succeeds
            // (spec: OC-8, EP-11). Swallowing it here is what keeps the layer
            // above from reading an errno to find out which it was.
            Err(gone) if gone == Errno::NOENT => Ok(()),
            Err(cause) => Err(self.folder.refused(name, LocalOperation::Removing, cause)),
        }
    }

    fn path_of(&self, name: &str) -> PathBuf {
        self.folder.path_of(name)
    }
}
