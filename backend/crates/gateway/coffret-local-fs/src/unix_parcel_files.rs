use std::ffi::OsString;
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

    async fn list(&self, dir: &Path) -> Result<Vec<OsString>, LocalIoError> {
        let refused = |cause| LocalIoError::new(LocalOperation::Listing, dir, cause);
        let mut entries = match fs::read_dir(dir).await {
            Ok(entries) => entries,
            // A device that never kept a parcel never made the directory, and
            // has nothing in it to list.
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(cause) => return Err(refused(cause)),
        };
        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(refused)? {
            names.push(entry.file_name());
        }
        Ok(names)
    }

    async fn size(&self, path: &Path) -> Result<Option<u64>, LocalIoError> {
        match fs::symlink_metadata(path).await {
            Ok(metadata) if metadata.is_file() => Ok(Some(metadata.len())),
            // Something that is not a file is not a kept parcel, whatever row
            // names it; the fetch meets it as not held (spec: PK-21).
            Ok(_) => Ok(None),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(cause) => Err(LocalIoError::new(LocalOperation::Stating, path, cause)),
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

#[cfg(test)]
mod tests {
    use coffret_usecase::ParcelFiles;

    use crate::unix_fs::UnixFs;

    // The sweep at open reads the directory's names, and the work answer reads
    // each row's file length: a directory never made lists nothing, and a file
    // that is gone has no length rather than failing (spec: PK-21).
    #[tokio::test]
    async fn lists_the_parcel_directory_and_sizes_its_files() {
        let state = tempfile::tempdir().expect("a temporary directory must be available");
        let dir = state.path().join("parcels");
        let fs = UnixFs::new();
        assert!(fs.list(&dir).await.expect("listing").is_empty());

        fs.prepare_dir(&dir).await.expect("making the directory");
        std::fs::write(dir.join("a-0.parcel"), [0u8; 5]).expect("writing a parcel");
        std::fs::create_dir(dir.join("not-a-file")).expect("making a folder");

        let mut names = fs.list(&dir).await.expect("listing");
        names.sort();
        assert_eq!(names, ["a-0.parcel", "not-a-file"]);
        assert_eq!(
            fs.size(&dir.join("a-0.parcel")).await.expect("sizing"),
            Some(5)
        );
        assert_eq!(
            fs.size(&dir.join("b-0.parcel")).await.expect("sizing"),
            None
        );
        assert_eq!(
            fs.size(&dir.join("not-a-file")).await.expect("sizing"),
            None
        );
    }
}
