use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::descent_error::DescentError;
use crate::flushed_file::FlushedFile;
use crate::in_memory_fs::in_memory_flushed_file::InMemoryFlushedFile;
use crate::in_memory_fs::state::{lock, State};
use crate::local_operation::LocalOperation;
use crate::scratch_file::ScratchFile;

/// One temporary file of [`InMemoryFs`](super::InMemoryFs), open for writing.
///
/// It writes into the map as it goes rather than buffering until the flush,
/// because that is what the device does: bytes reach the file as they are
/// written, and the flush is only what makes them outlast the process. A case
/// that stops a run at [`Flushing`](LocalOperation::Flushing) therefore finds a
/// half-written scratch file in the fake and no file at the Entry's own name,
/// which is exactly the state EP-11's ordering promises.
///
/// There is no separate "durable" bit, and there is nothing for one to say:
/// what makes a flush observable above this capability is that a
/// [`FlushedFile`] is the only thing that can be published, and a fake has no
/// process to lose the unflushed bytes of. What a case scripts instead is the
/// flush *refusing*, which is the state the ordering is about.
pub(super) struct InMemoryScratchFile {
    state: Arc<Mutex<State>>,
    /// Where the temporary file stands.
    path: PathBuf,
    /// Where the file will stand once it is published.
    final_path: PathBuf,
}

impl InMemoryScratchFile {
    /// A writer over the file `path` of the fake behind `state`, bound for
    /// `final_path`.
    pub(super) fn new(state: Arc<Mutex<State>>, path: PathBuf, final_path: PathBuf) -> Self {
        Self {
            state,
            path,
            final_path,
        }
    }
}

#[async_trait]
impl ScratchFile for InMemoryScratchFile {
    async fn write(&mut self, bytes: &[u8]) -> Result<(), DescentError> {
        let mut state = lock(&self.state);
        state
            .attempt(LocalOperation::Writing, &self.path)
            .map_err(DescentError::Io)?;
        state.append(&self.path, bytes);
        Ok(())
    }

    async fn flush(self: Box<Self>) -> Result<Box<dyn FlushedFile>, DescentError> {
        lock(&self.state)
            .attempt(LocalOperation::Flushing, &self.path)
            .map_err(DescentError::Io)?;
        Ok(Box::new(InMemoryFlushedFile::new(
            Arc::clone(&self.state),
            self.path.clone(),
            self.final_path.clone(),
        )))
    }
}
