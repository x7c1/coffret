use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::below_root_error::BelowRootError;
use crate::in_memory_fs::state::{lock, State};
use crate::kept_file::KeptFile;
use crate::local_operation::LocalOperation;

/// A file of [`InMemoryFs`](super::InMemoryFs) kept aside under a scratch
/// name, for the add about to write over it (spec: LA-36).
///
/// Both calls name a file inside the folder the destination that made it was
/// holding, as that destination's own calls do (spec: EP-4, EP-11).
pub(super) struct InMemoryKeptFile {
    state: Arc<Mutex<State>>,
    /// The scratch name it is kept under.
    kept: PathBuf,
    /// The file's own name, which it goes back to.
    at: PathBuf,
}

impl InMemoryKeptFile {
    /// The file kept at `kept` for the name `at`, in the fake behind `state`.
    pub(super) fn new(state: Arc<Mutex<State>>, kept: PathBuf, at: PathBuf) -> Self {
        Self { state, kept, at }
    }
}

impl KeptFile for InMemoryKeptFile {
    fn put_back(&self) -> Result<(), BelowRootError> {
        let mut state = lock(&self.state);
        state
            .attempt(LocalOperation::Renaming, &self.at)
            .map_err(BelowRootError::Io)?;
        state.move_back(&self.kept, &self.at);
        Ok(())
    }

    fn remove(&self) -> Result<(), BelowRootError> {
        let mut state = lock(&self.state);
        state
            .attempt(LocalOperation::Removing, &self.kept)
            .map_err(BelowRootError::Io)?;
        state.remove_kept(&self.kept);
        Ok(())
    }
}
