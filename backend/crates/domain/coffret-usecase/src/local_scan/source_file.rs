use std::path::PathBuf;

use coffret_model::{EntryPath, Mtime};
use tokio::fs;
use tokio::io::AsyncReadExt;

use crate::local_error::LocalError;
use crate::local_operation::LocalOperation;

/// One local file the scan found, at the Library position it stands for.
///
/// The three observed values are what a filesystem answers cheaply, and they
/// are the whole of what a scan compares against
/// [`LocalObservation`](crate::device_state::LocalObservation) before deciding
/// to read a file at all: a file whose length and modification time are what
/// this device last saw is not opened (spec: EP-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceFile {
    /// The Library position the file stands at, derived from the mapping it was
    /// found under (spec: EP-9).
    pub(crate) path: EntryPath,
    /// Where the file is on this device.
    ///
    /// Device state and nothing else: it never travels into a Container, a
    /// Journal record, or a log line.
    pub(crate) local_path: PathBuf,
    /// The file's length in bytes when the scan looked.
    pub(crate) size: u64,
    /// The file's modification time when the scan looked, which is the value
    /// the Entry carries (spec: FM-9).
    pub(crate) mtime: Mtime,
}

impl SourceFile {
    /// The file's whole plaintext.
    ///
    /// For the steps that can afford it: a sync's scan hashes a candidate to
    /// settle whether it really changed, and its spool encodes one file into a
    /// Container of its own. A Pack cannot be read this way, which is what
    /// [`open`](Self::open) is for.
    pub(crate) async fn read(&self) -> Result<Vec<u8>, LocalError> {
        fs::read(&self.local_path)
            .await
            .map_err(|cause| LocalError::io(LocalOperation::Reading, &self.local_path, cause))
    }

    /// Opens the file to be walked a buffer at a time.
    ///
    /// What a Pack does with every file it holds — hashing it before the entry
    /// table is written, and feeding it through the encoder afterwards — so
    /// neither step is bounded by what fits in memory (spec: PK-3, PK-5).
    pub(crate) async fn open(&self) -> Result<SourceReader<'_>, LocalError> {
        let file = fs::File::open(&self.local_path)
            .await
            .map_err(|cause| LocalError::io(LocalOperation::Reading, &self.local_path, cause))?;
        Ok(SourceReader {
            file,
            local_path: &self.local_path,
        })
    }
}

/// One open local file, handing its plaintext over a buffer at a time.
pub(crate) struct SourceReader<'a> {
    file: fs::File,
    local_path: &'a PathBuf,
}

impl SourceReader<'_> {
    /// Fills `buffer` with the next stretch of the file.
    ///
    /// Zero means the file is exhausted, which is the only way a caller learns
    /// how long the file turned out to be — a length the scan's `stat` may no
    /// longer agree with.
    pub(crate) async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, LocalError> {
        self.file
            .read(buffer)
            .await
            .map_err(|cause| LocalError::io(LocalOperation::Reading, self.local_path, cause))
    }
}
