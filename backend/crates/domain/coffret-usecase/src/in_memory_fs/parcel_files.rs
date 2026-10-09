use std::io;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use crate::in_memory_fs::in_memory_writer::InMemoryWriter;
use crate::in_memory_fs::state::lock;
use crate::in_memory_fs::InMemoryFs;
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;
use crate::parcel_files::ParcelFiles;
use crate::spool_writer::SpoolWriter;

/// The kept parcels are files on the same fake disk the spool is on, written
/// through the same writer and failing at the same scripted steps: one device
/// has one disk (spec: PK-21).
#[async_trait]
impl ParcelFiles for InMemoryFs {
    async fn prepare_dir(&self, dir: &Path) -> Result<(), LocalIoError> {
        lock(&self.state).prepare_dir(dir);
        Ok(())
    }

    async fn create(&self, path: &Path) -> Result<Box<dyn SpoolWriter>, LocalIoError> {
        let mut state = lock(&self.state);
        state.attempt(LocalOperation::Creating, path)?;
        state.create(path)?;
        drop(state);
        Ok(Box::new(InMemoryWriter::new(
            Arc::clone(&self.state),
            path.to_path_buf(),
        )))
    }

    async fn reader(
        &self,
        path: &Path,
    ) -> Result<Option<Box<dyn AsyncRead + Send + Unpin>>, LocalIoError> {
        let mut state = lock(&self.state);
        state.attempt(LocalOperation::Reading, path)?;
        Ok(state
            .content(path)
            .map(|content| Box::new(io::Cursor::new(content)) as Box<dyn AsyncRead + Send + Unpin>))
    }

    async fn discard(&self, path: &Path) -> Result<(), LocalIoError> {
        let mut state = lock(&self.state);
        state.attempt(LocalOperation::Removing, path)?;
        state.remove(path);
        Ok(())
    }
}
