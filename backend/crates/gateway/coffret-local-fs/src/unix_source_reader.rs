use std::path::PathBuf;

use async_trait::async_trait;
use coffret_usecase::{LocalIoError, LocalOperation, SourceReader};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

/// One local file of [`UnixFs`](crate::UnixFs), open for reading.
///
/// It keeps the path alongside the handle because a refusal has to name the file
/// it was refused for, and an open handle no longer knows: the three parts of a
/// [`LocalIoError`] are settled where the call is made.
pub(crate) struct UnixSourceReader {
    file: File,
    path: PathBuf,
    bytes: u64,
}

impl UnixSourceReader {
    /// A reader over `file`, which was opened at `path`.
    pub(crate) fn new(file: File, path: PathBuf, bytes: u64) -> Self {
        Self { file, path, bytes }
    }
}

#[async_trait]
impl SourceReader for UnixSourceReader {
    fn len(&self) -> u64 {
        self.bytes
    }

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, LocalIoError> {
        self.file
            .read(buffer)
            .await
            .map_err(|cause| LocalIoError::new(LocalOperation::Reading, &self.path, cause))
    }
}
