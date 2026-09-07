use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use coffret_model::Mtime;
use coffret_usecase::{DescentError, FlushedFile, LocalIoError, LocalOperation};

use crate::local_times::system_time_of;
use crate::unix_destinations::open_folder::OpenFolder;

/// One temporary file on this device's disk whose bytes are on the device,
/// waiting for its final name.
///
/// The handle is kept until the stamp takes it, because setting a file's times
/// on the handle a run has been writing to is both one syscall fewer than
/// reopening the name and the only form that keeps the confinement: reopening by
/// path would be the one place a symbolic link could get between the descent and
/// the stamp.
pub(crate) struct UnixFlushedFile {
    folder: Arc<OpenFolder>,
    scratch_name: String,
    path: PathBuf,
    /// The open file until the stamp spends it, which is the last thing that
    /// needs it: the rename below names the file rather than holding it.
    file: Option<std::fs::File>,
}

impl UnixFlushedFile {
    /// The flushed file `scratch_name` inside `folder`, still open as `file`.
    pub(super) fn new(
        folder: Arc<OpenFolder>,
        scratch_name: String,
        path: PathBuf,
        file: std::fs::File,
    ) -> Self {
        Self {
            folder,
            scratch_name,
            path,
            file: Some(file),
        }
    }

    /// What the operating system refused about the stamp.
    fn refused(&self, cause: io::Error) -> DescentError {
        DescentError::Io(LocalIoError::new(
            LocalOperation::Stamping,
            &self.path,
            cause,
        ))
    }
}

#[async_trait::async_trait]
impl FlushedFile for UnixFlushedFile {
    async fn stamp(&mut self, mtime: Mtime) -> Result<(), DescentError> {
        let modified = system_time_of(mtime).ok_or_else(|| {
            self.refused(io::Error::new(
                io::ErrorKind::InvalidInput,
                "an Entry's modification time this platform's clock cannot reach",
            ))
        })?;
        let file = self
            .file
            .take()
            .expect("a flushed file is stamped exactly once");

        // Off the runtime's threads, because setting times is a blocking
        // metadata call on a handle and the runtime's file API offers none.
        tokio::task::spawn_blocking(move || {
            file.set_times(std::fs::FileTimes::new().set_modified(modified))
        })
        .await
        .map_err(|joined| self.refused(io::Error::other(joined)))?
        .map_err(|cause| self.refused(cause))
    }

    fn publish(self: Box<Self>) -> Result<(), DescentError> {
        // Both names are resolved against the open folder, so the rename lands
        // where the descent arrived whatever has happened to the path above it
        // since. A rename within one directory is atomic, which is what makes it
        // the moment the file exists (spec: EP-11).
        rustix::fs::renameat(
            self.folder.directory(),
            self.scratch_name.as_str(),
            self.folder.directory(),
            self.folder.name(),
        )
        .map_err(|cause| {
            self.folder
                .refused(self.folder.name(), LocalOperation::Renaming, cause)
        })
    }
}
