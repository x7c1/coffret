use std::io::ErrorKind;
use std::path::Path;

use async_trait::async_trait;
use coffret_usecase::{LocalIoError, LocalOperation, ParcelFiles, SpoolWriter};
use tokio::fs;
use tokio::io::AsyncRead;
use tracing::debug;

use crate::unix_fs::UnixFs;
use crate::unix_spool_writer::UnixSpoolWriter;

/// The parcels a fetch keeps, as files in a directory under the state
/// directory beside the spool (spec: PK-21).
///
/// The same calls the spool makes, for the same kind of file: ciphertext this
/// device wrote and will read back, flushed to the device before the writer is
/// spent. Where the directory is, is the composition root's to say.
#[async_trait]
impl ParcelFiles for UnixFs {
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

    async fn reader(
        &self,
        path: &Path,
    ) -> Result<Option<Box<dyn AsyncRead + Send + Unpin>>, LocalIoError> {
        match fs::File::open(path).await {
            Ok(file) => Ok(Some(Box::new(file))),
            // A row naming a file that is not there is the case the fetch meets
            // as a parcel not held (spec: PK-21), so absence is an answer here
            // rather than a failure — read off the errno in this crate, which
            // is the one place that reads one.
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(cause) => Err(LocalIoError::new(LocalOperation::Reading, path, cause)),
        }
    }

    async fn discard(&self, path: &Path) -> Result<(), LocalIoError> {
        match fs::remove_file(path).await {
            Ok(()) => Ok(()),
            // Gone already is what letting go wanted (spec: OC-8, PK-21).
            Err(error) if error.kind() == ErrorKind::NotFound => {
                debug!("a kept parcel was already gone when it was let go");
                Ok(())
            }
            Err(cause) => Err(LocalIoError::new(LocalOperation::Removing, path, cause)),
        }
    }
}
