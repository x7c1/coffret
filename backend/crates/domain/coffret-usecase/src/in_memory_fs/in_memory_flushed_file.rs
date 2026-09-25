use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;

use crate::below_root_error::BelowRootError;
use crate::flushed_file::FlushedFile;
use crate::in_memory_fs::state::{lock, State};
use crate::local_operation::LocalOperation;

/// One scratch of [`InMemoryFs`](super::InMemoryFs) whose bytes are on the
/// device, waiting for its final name.
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
}

#[async_trait]
impl FlushedFile for InMemoryFlushedFile {
    async fn stamp(&mut self, modified: SystemTime) -> Result<(), BelowRootError> {
        let mut state = lock(&self.state);
        state
            .attempt(LocalOperation::Stamping, &self.path)
            .map_err(BelowRootError::Io)?;
        state.set_mtime(&self.path, whole_seconds(modified));
        Ok(())
    }

    fn publish(self: Box<Self>) -> Result<(), BelowRootError> {
        let mut state = lock(&self.state);
        state
            .attempt(LocalOperation::Renaming, &self.path)
            .map_err(BelowRootError::Io)?;
        state.rename(&self.path, &self.final_path);
        Ok(())
    }
}

/// A moment as the whole seconds from the Unix epoch the fake keeps a time in.
///
/// Exact for every moment a placement stamps, since those come from an Entry's
/// own whole-second time; saturating at the ends for anything else, which no
/// case hands it.
fn whole_seconds(at: SystemTime) -> i64 {
    match at.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_secs()).map_or(i64::MIN, |secs| -secs),
    }
}
