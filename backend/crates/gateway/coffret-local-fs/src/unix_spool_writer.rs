use std::path::PathBuf;

use async_trait::async_trait;
use coffret_usecase::{LocalIoError, LocalOperation, SpoolWriter};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// One spool file on this device's disk, open for writing.
///
/// It keeps the path beside the handle for one reason: a refusal has to say
/// which file it is about, and an open handle no longer knows. The path stays in
/// the error's value and out of its message, which is where a local path may
/// never appear in a diagnostic event (spec: EL-1).
pub(crate) struct UnixSpoolWriter {
    file: File,
    path: PathBuf,
}

impl UnixSpoolWriter {
    /// A writer over an open file at `path`.
    pub(crate) fn new(file: File, path: PathBuf) -> Self {
        Self { file, path }
    }
}

#[async_trait]
impl SpoolWriter for UnixSpoolWriter {
    async fn write(&mut self, bytes: &[u8]) -> Result<(), LocalIoError> {
        self.file
            .write_all(bytes)
            .await
            .map_err(|cause| LocalIoError::new(LocalOperation::Writing, &self.path, cause))
    }

    async fn finish(mut self: Box<Self>) -> Result<(), LocalIoError> {
        // To the device and not merely to the operating system: what a spool is
        // for is to still be there after the run that wrote it is not
        // (spec: OC-2). `sync_all` flushes the buffered writes on the way, so
        // there is nothing to flush first.
        self.file
            .sync_all()
            .await
            .map_err(|cause| LocalIoError::new(LocalOperation::Flushing, &self.path, cause))
    }
}
