use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::io::AsyncRead;

use crate::in_memory_fs::in_memory_writer::InMemoryWriter;
use crate::in_memory_fs::state::State;
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;
use crate::spool::Spool;
use crate::spool_writer::SpoolWriter;

mod in_memory_writer;

mod state;

/// A [`Spool`] that keeps everything in memory, for tests.
///
/// It stands to [`Spool`] as [`InMemoryStore`](crate::InMemoryStore) stands to
/// [`ObjectStore`](crate::ObjectStore), and it earns its place for one reason
/// beyond needing no directory: it can be told to fail. The rules a sync and a
/// freeze keep around the spool are rules about interruption, which [`Spool`]
/// states (spec: OC-2, OC-6), and a real filesystem cannot be asked to refuse a
/// chosen step. [`fail_on`](Self::fail_on) is what asks.
///
/// What it models of a filesystem is only what the spool lifecycle can tell
/// apart: which directories were made, what each file holds, and the fact that a
/// file cannot be created under a directory nobody made. There are no
/// permissions, no links, and no listing — nothing in the flows lists the spool
/// directory, because the pending rows are its only index.
#[derive(Debug, Default)]
pub struct InMemoryFs {
    // Behind an `Arc` because a writer outlives the call that handed it over and
    // writes into the same fake, the way a file handle does.
    state: Arc<Mutex<State>>,
}

impl InMemoryFs {
    /// An empty filesystem: no directories and no files.
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes the `nth` (1-based) invocation of `operation` fail.
    ///
    /// The five operations a spool lifecycle performs are the ones worth
    /// scripting: [`Creating`](LocalOperation::Creating) for
    /// [`create`](Spool::create), [`Writing`](LocalOperation::Writing) and
    /// [`Flushing`](LocalOperation::Flushing) for the writer it hands back,
    /// [`Reading`](LocalOperation::Reading) for [`open`](Spool::open), and
    /// [`Removing`](LocalOperation::Removing) for
    /// [`discard`](Spool::discard). The count is per operation, so scripting the
    /// second `Writing` fails the second write whatever else the run did in
    /// between.
    ///
    /// [`prepare_dir`](Spool::prepare_dir) is deliberately not counted or
    /// scripted: it is one call at the top of a run, and a case that wants the
    /// spool directory to be missing simply never prepares it — which is what a
    /// device with no such directory does to
    /// [`create`](Spool::create) anyway.
    ///
    /// The refusal's cause carries [`io::ErrorKind::Other`], because no
    /// operating system reported it: what the case is about is the operation
    /// that failed, never which errno stood behind it.
    pub fn fail_on(&self, operation: LocalOperation, nth: usize) {
        self.state().fail_on(operation, nth);
    }

    /// The files directly under `dir`, in path order.
    ///
    /// What a case counting spools reads: a run that committed its batch leaves
    /// none, and one that was interrupted leaves exactly the ones its pending
    /// rows name (spec: OC-2).
    pub fn files_under(&self, dir: &Path) -> Vec<PathBuf> {
        self.state().files_under(dir)
    }

    /// One file's whole content, or `None` where nothing is at that path.
    pub fn content(&self, path: &Path) -> Option<Vec<u8>> {
        self.state().content(path)
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
