use std::io;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use crate::in_memory_fs::in_memory_writer::InMemoryWriter;
use crate::in_memory_fs::InMemoryFs;
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;
use crate::spool::Spool;
use crate::spool_writer::SpoolWriter;

#[async_trait]
impl Spool for InMemoryFs {
    async fn prepare_dir(&self, dir: &Path) -> Result<(), LocalIoError> {
        self.state().prepare_dir(dir);
        Ok(())
    }

    async fn create(&self, path: &Path) -> Result<Box<dyn SpoolWriter>, LocalIoError> {
        let mut state = self.state();
        state.attempt(LocalOperation::Creating, path)?;
        state.create(path)?;
        drop(state);
        Ok(Box::new(InMemoryWriter::new(
            Arc::clone(&self.state),
            path.to_path_buf(),
        )))
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn AsyncRead + Send + Unpin>, LocalIoError> {
        let mut state = self.state();
        state.attempt(LocalOperation::Reading, path)?;
        let content = state.content(path).ok_or_else(|| {
            LocalIoError::new(
                LocalOperation::Reading,
                path,
                io::Error::new(io::ErrorKind::NotFound, "no spool file is at this path"),
            )
        })?;
        Ok(Box::new(io::Cursor::new(content)))
    }

    async fn discard(&self, path: &Path) -> Result<(), LocalIoError> {
        let mut state = self.state();
        state.attempt(LocalOperation::Removing, path)?;
        state.remove(path);
        Ok(())
    }
}
