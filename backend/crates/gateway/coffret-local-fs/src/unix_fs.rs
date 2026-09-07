use std::io::ErrorKind;
use std::path::Path;

use async_trait::async_trait;
use coffret_usecase::{LocalIoError, LocalOperation, Spool, SpoolWriter};
use tokio::fs;
use tokio::io::AsyncRead;
use tracing::debug;

use crate::unix_spool_writer::UnixSpoolWriter;

/// The device's own filesystem.
///
/// It holds nothing: every call names the path it is about, so one of these
/// serves a whole process however many Libraries it has open. The name says what
/// it stands for — the ordinary filesystem under a Unix-like operating system —
/// rather than what it implements, because a device that needed different calls
/// would be a second provider here rather than a change to this one.
///
/// It answers both capabilities the flows reach this disk through:
/// [`Spool`] here, and [`MappedRoots`](coffret_usecase::MappedRoots) beside it.
/// One type for both because a device has one disk — a composition root hands
/// the same value to a request's two fields, and a case that scripts a folder
/// which will not list and a spool which will not flush scripts one thing.
#[derive(Debug, Default)]
pub struct UnixFs;

impl UnixFs {
    /// The filesystem this process is running on.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Spool for UnixFs {
    async fn prepare_dir(&self, dir: &Path) -> Result<(), LocalIoError> {
        fs::create_dir_all(dir)
            .await
            .map_err(|cause| LocalIoError::new(LocalOperation::Creating, dir, cause))
    }

    async fn create(&self, path: &Path) -> Result<Box<dyn SpoolWriter>, LocalIoError> {
        let file = fs::File::create(path)
            .await
            .map_err(|cause| LocalIoError::new(LocalOperation::Creating, path, cause))?;
        Ok(Box::new(UnixSpoolWriter::new(file, path.to_path_buf())))
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn AsyncRead + Send + Unpin>, LocalIoError> {
        let file = fs::File::open(path)
            .await
            .map_err(|cause| LocalIoError::new(LocalOperation::Reading, path, cause))?;
        Ok(Box::new(file))
    }

    async fn discard(&self, path: &Path) -> Result<(), LocalIoError> {
        match fs::remove_file(path).await {
            Ok(()) => Ok(()),
            // Absence is the outcome the caller wanted, and it is an *ordinary*
            // outcome rather than only a repeated one: a pending row is written
            // before the file it names, so a row can name a spool whose creation
            // never happened (spec: OC-2, OC-6). Swallowing it here is what
            // keeps the layer above from reading an `ErrorKind` to find out
            // which of the two it was.
            Err(error) if error.kind() == ErrorKind::NotFound => {
                debug!("a spool file was already gone when it was discarded");
                Ok(())
            }
            Err(cause) => Err(LocalIoError::new(LocalOperation::Removing, path, cause)),
        }
    }
}
