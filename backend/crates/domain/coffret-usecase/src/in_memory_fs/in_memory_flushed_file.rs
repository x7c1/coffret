use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use coffret_model::Mtime;

use crate::descent_error::DescentError;
use crate::flushed_file::FlushedFile;
use crate::in_memory_fs::state::State;
use crate::local_operation::LocalOperation;

/// One temporary file of [`InMemoryFs`](super::InMemoryFs) whose bytes are on
/// the device, waiting for its final name.
///
/// It works against the same state the writer before it did, so what a case
/// reads back after a publish is the file the writer wrote — under the name the
/// descent was given.
pub(super) struct InMemoryFlushedFile {
    state: Arc<Mutex<State>>,
    path: PathBuf,
    final_path: PathBuf,
}

impl InMemoryFlushedFile {
    /// The flushed file at `path` of the fake behind `state`, bound for
    /// `final_path`.
    pub(super) fn new(state: Arc<Mutex<State>>, path: PathBuf, final_path: PathBuf) -> Self {
        Self {
            state,
            path,
            final_path,
        }
    }

    /// The fake's state, taken even from a lock a panicking case poisoned: what
    /// is behind it is a case's own bookkeeping, and a poisoned lock would
    /// replace the failure that panicked with one about the lock.
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl FlushedFile for InMemoryFlushedFile {
    async fn stamp(&mut self, mtime: Mtime) -> Result<(), DescentError> {
        let mut state = self.state();
        state
            .attempt(LocalOperation::Stamping, &self.path)
            .map_err(DescentError::Io)?;
        state.set_mtime(&self.path, mtime.as_unix_seconds());
        Ok(())
    }

    fn publish(self: Box<Self>) -> Result<(), DescentError> {
        let mut state = self.state();
        state
            .attempt(LocalOperation::Renaming, &self.path)
            .map_err(DescentError::Io)?;
        state.rename(&self.path, &self.final_path);
        Ok(())
    }
}
