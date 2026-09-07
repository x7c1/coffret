use std::path::PathBuf;
use std::sync::Arc;

use coffret_usecase::{DescentError, FlushedFile, LocalIoError, LocalOperation, ScratchFile};
use tokio::io::AsyncWriteExt;

use crate::unix_destinations::open_folder::OpenFolder;
use crate::unix_destinations::unix_flushed_file::UnixFlushedFile;

/// One temporary file on this device's disk, open for writing.
///
/// It keeps the path beside the handle for one reason: a refusal has to say
/// which file it is about, and an open handle no longer knows. The path stays in
/// the error's value and out of its message, which is where a local path may
/// never appear (spec: EP-1).
pub(crate) struct UnixScratchFile {
    folder: Arc<OpenFolder>,
    scratch_name: String,
    path: PathBuf,
    file: tokio::fs::File,
}

impl UnixScratchFile {
    /// A writer over `file`, which was created as `scratch_name` inside
    /// `folder`.
    pub(super) fn new(
        folder: Arc<OpenFolder>,
        scratch_name: String,
        file: tokio::fs::File,
    ) -> Self {
        let path = folder.path_of(&scratch_name);
        Self {
            folder,
            scratch_name,
            path,
            file,
        }
    }
}

#[async_trait::async_trait]
impl ScratchFile for UnixScratchFile {
    async fn write(&mut self, bytes: &[u8]) -> Result<(), DescentError> {
        self.file.write_all(bytes).await.map_err(|cause| {
            DescentError::Io(LocalIoError::new(
                LocalOperation::Writing,
                &self.path,
                cause,
            ))
        })
    }

    async fn flush(self: Box<Self>) -> Result<Box<dyn FlushedFile>, DescentError> {
        let Self {
            folder,
            scratch_name,
            path,
            file,
        } = *self;

        // To the device and not merely to the operating system, because the
        // rename behind this is what publishes the file: a name that appeared
        // before its content reached the disk would promise bytes a crash then
        // lost (spec: EP-11). `sync_all` flushes the buffered writes on the way,
        // so there is nothing to flush first.
        file.sync_all().await.map_err(|cause| {
            DescentError::Io(LocalIoError::new(LocalOperation::Flushing, &path, cause))
        })?;

        // Handed over as a blocking handle, because what is left to do to it is
        // a metadata call the runtime's file API does not offer.
        Ok(Box::new(UnixFlushedFile::new(
            folder,
            scratch_name,
            path,
            file.into_std().await,
        )))
    }
}
