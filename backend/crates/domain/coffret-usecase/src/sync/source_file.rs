use std::path::PathBuf;

use coffret_model::{EntryPath, Mtime};
use tokio::fs;

use crate::local_operation::LocalOperation;
use crate::sync::sync_error::{SyncError, SyncResult};

/// One local file the scan found, at the Library position it stands for.
///
/// The three observed values are what a filesystem answers cheaply, and they
/// are the whole of what a scan compares against
/// [`LocalObservation`](crate::device_state::LocalObservation) before deciding
/// to read a file at all: a file whose length and modification time are what
/// this device last saw is not opened (spec: EP-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceFile {
    /// The Library position the file stands at, derived from the mapping it was
    /// found under (spec: EP-9).
    pub(super) path: EntryPath,
    /// Where the file is on this device.
    ///
    /// Device state and nothing else: it never travels into a Container, a
    /// Journal record, or a log line.
    pub(super) local_path: PathBuf,
    /// The file's length in bytes when the scan looked.
    pub(super) size: u64,
    /// The file's modification time when the scan looked, which is the value
    /// the Entry carries (spec: FM-9).
    pub(super) mtime: Mtime,
}

impl SourceFile {
    /// The file's whole plaintext.
    ///
    /// Both steps that open a source file want all of it — the scan hashes it
    /// to settle whether a candidate really changed, the spool encodes it — so
    /// the read belongs to the file rather than to either of them.
    pub(super) async fn read(&self) -> SyncResult<Vec<u8>> {
        fs::read(&self.local_path)
            .await
            .map_err(|cause| SyncError::Io {
                operation: LocalOperation::Reading,
                path: self.local_path.clone(),
                cause,
            })
    }
}
