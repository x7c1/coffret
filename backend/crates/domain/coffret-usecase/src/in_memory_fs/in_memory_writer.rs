use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::in_memory_fs::state::{lock, State};
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;
use crate::spool_writer::SpoolWriter;

/// One spool file of [`InMemoryFs`](super::InMemoryFs), open for writing.
///
/// It writes into the map as it goes rather than buffering until the flush,
/// because that is what the device does: bytes reach the file as they are
/// written, and the flush is only what makes them outlast the process. A case
/// that stops a run at [`Flushing`](LocalOperation::Flushing) therefore finds
/// the ciphertext in the fake and the pending row still
/// [`Spooling`](crate::device_state::SpoolState::Spooling), which is exactly the
/// state OC-2's ordering promises.
pub(super) struct InMemoryWriter {
    state: Arc<Mutex<State>>,
    path: PathBuf,
}

impl InMemoryWriter {
    /// A writer over the file `path` of the fake behind `state`.
    pub(super) fn new(state: Arc<Mutex<State>>, path: PathBuf) -> Self {
        Self { state, path }
    }
}

#[async_trait]
impl SpoolWriter for InMemoryWriter {
    async fn write(&mut self, bytes: &[u8]) -> Result<(), LocalIoError> {
        let mut state = lock(&self.state);
        state.attempt(LocalOperation::Writing, &self.path)?;
        state.append(&self.path, bytes);
        Ok(())
    }

    async fn finish(self: Box<Self>) -> Result<(), LocalIoError> {
        lock(&self.state).attempt(LocalOperation::Flushing, &self.path)
    }
}
