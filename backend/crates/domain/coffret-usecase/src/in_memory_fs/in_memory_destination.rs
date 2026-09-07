use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::descent_error::DescentError;
use crate::destination::Destination;
use crate::in_memory_fs::in_memory_scratch_file::InMemoryScratchFile;
use crate::in_memory_fs::state::State;
use crate::local_operation::LocalOperation;
use crate::scratch_file::ScratchFile;

/// One destination folder of [`InMemoryFs`](super::InMemoryFs), reached and held
/// open.
///
/// "Held open" is the fake's stand-in for the real thing's open directory: it
/// keeps the folder's path and works against the shared state through it, which
/// is as close as something with no file descriptors comes to a handle. What it
/// models faithfully is the part that matters above the capability — every write
/// names a file *inside this folder* and the folder itself is never resolved
/// again (spec: EP-4, EP-11).
pub(super) struct InMemoryDestination {
    state: Arc<Mutex<State>>,
    folder: PathBuf,
    /// What the file itself is called inside that folder.
    name: String,
}

impl InMemoryDestination {
    /// The folder `folder` of the fake behind `state`, for a file called `name`.
    pub(super) fn new(state: Arc<Mutex<State>>, folder: PathBuf, name: String) -> Self {
        Self {
            state,
            folder,
            name,
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

impl Destination for InMemoryDestination {
    fn create(&self, scratch_name: &str) -> Result<Box<dyn ScratchFile>, DescentError> {
        let path = self.folder.join(scratch_name);
        let mut state = self.state();
        state
            .attempt(LocalOperation::Creating, &path)
            .map_err(DescentError::Io)?;
        state.create_new(&path)?;
        drop(state);
        Ok(Box::new(InMemoryScratchFile::new(
            Arc::clone(&self.state),
            path,
            self.folder.join(&self.name),
        )))
    }

    fn remove(&self, name: &str) -> Result<(), DescentError> {
        let path = self.folder.join(name);
        let mut state = self.state();
        state
            .attempt(LocalOperation::Removing, &path)
            .map_err(DescentError::Io)?;
        // Absence is the outcome the caller wanted, here as in the spool
        // (spec: OC-6, EP-11), so nothing is checked before the removal.
        state.remove(&path);
        Ok(())
    }

    fn path_of(&self, name: &str) -> PathBuf {
        self.folder.join(name)
    }
}
