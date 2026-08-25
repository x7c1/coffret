use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use coffret_model::{ContainerId, ContainerSummary, EntryLocation, EntryPath, IndexCheckpoint};
use coffret_usecase::device_state::{
    DeviceTime, LocalEntry, LocalObservation, Mapping, PendingUpload,
};
use coffret_usecase::{
    CommittedBatch, Index, IndexError, IndexResult, JournalRecord, SnapshotContent,
};
use rusqlite::Connection;

use crate::error::translate;
use crate::{device_state, library_state, schema};

/// One Library's [`Index`], kept in one SQLite file.
///
/// SQLite is synchronous and the port is not, so every operation runs on a
/// blocking thread against a single connection held under a lock. One
/// connection is enough because the catalog serves one device: the lock is
/// never the contended thing, and a second connection would only bring SQLite's
/// own writer contention into a process that has no second writer.
///
/// Each operation runs in one transaction, so a rejected replay leaves the
/// catalog exactly as it was — the all-or-nothing a commit means (spec: CP-1).
pub struct SqliteIndex {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteIndex {
    /// Opens the catalog kept in the file at `path`, creating it if there is
    /// none.
    ///
    /// A file whose layout this build does not know is refused with
    /// [`IndexError::UnsupportedSchema`] rather than migrated: the catalog can
    /// be rebuilt from Storage, so discarding an unreadable one is cheaper and
    /// safer than converting it (spec: RV-5).
    pub fn open(path: impl AsRef<Path>) -> IndexResult<Self> {
        let connection = Connection::open(path).map_err(translate("opening the Index file"))?;
        schema::prepare(&connection)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /// Runs a read on a blocking thread.
    async fn read<T>(
        &self,
        operation: &'static str,
        work: impl FnOnce(&Connection) -> IndexResult<T> + Send + 'static,
    ) -> IndexResult<T>
    where
        T: Send + 'static,
    {
        let connection = Arc::clone(&self.connection);
        join(
            operation,
            tokio::task::spawn_blocking(move || work(&locked(&connection))),
        )
        .await
    }

    /// Runs a write on a blocking thread, in one transaction.
    async fn write<T>(
        &self,
        operation: &'static str,
        work: impl FnOnce(&Connection) -> IndexResult<T> + Send + 'static,
    ) -> IndexResult<T>
    where
        T: Send + 'static,
    {
        let connection = Arc::clone(&self.connection);
        join(
            operation,
            tokio::task::spawn_blocking(move || {
                let mut guard = locked(&connection);
                let transaction = guard.transaction().map_err(translate(operation))?;
                let outcome = work(&transaction)?;
                transaction.commit().map_err(translate(operation))?;
                Ok(outcome)
            }),
        )
        .await
    }
}

/// Takes the connection, and takes it back after a panic.
///
/// A task that panicked mid-operation left its transaction unfinished, and
/// SQLite rolls an unfinished transaction back when it is dropped, so what is
/// behind the lock is a whole catalog either way.
fn locked(connection: &Mutex<Connection>) -> MutexGuard<'_, Connection> {
    connection
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Waits for the blocking thread, reporting a thread that never finished.
async fn join<T>(
    operation: &'static str,
    handle: tokio::task::JoinHandle<IndexResult<T>>,
) -> IndexResult<T> {
    match handle.await {
        Ok(outcome) => outcome,
        Err(error) => Err(IndexError::Backend {
            operation,
            cause: Box::new(error),
        }),
    }
}

#[async_trait]
impl Index for SqliteIndex {
    async fn restore(&self, snapshot: SnapshotContent) -> IndexResult<()> {
        self.write("restoring from a Snapshot", move |connection| {
            library_state::restore(connection, snapshot)
        })
        .await
    }

    async fn apply(&self, record: JournalRecord) -> IndexResult<()> {
        self.write("replaying a Journal record", move |connection| {
            library_state::apply(connection, record)
        })
        .await
    }

    async fn refresh(&self, batch: CommittedBatch) -> IndexResult<()> {
        self.write("applying this device's own commit", move |connection| {
            library_state::refresh(connection, batch)
        })
        .await
    }

    async fn snapshot(&self) -> IndexResult<SnapshotContent> {
        self.read("reading the catalog", library_state::snapshot)
            .await
    }

    async fn checkpoint(&self) -> IndexResult<Option<IndexCheckpoint>> {
        self.read("reading the checkpoint", library_state::checkpoint)
            .await
    }

    async fn entry_at(&self, path: &EntryPath) -> IndexResult<Option<EntryLocation>> {
        let path = path.clone();
        self.read("reading an Entry", move |connection| {
            library_state::entry_at(connection, &path)
        })
        .await
    }

    async fn entries_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<EntryLocation>> {
        let prefix = prefix.cloned();
        self.read("reading a subtree's Entries", move |connection| {
            library_state::entries_under(connection, prefix.as_ref())
        })
        .await
    }

    async fn containers_under(
        &self,
        prefix: Option<&EntryPath>,
    ) -> IndexResult<Vec<ContainerSummary>> {
        let prefix = prefix.cloned();
        self.read("reading a subtree's Containers", move |connection| {
            library_state::containers_under(connection, prefix.as_ref())
        })
        .await
    }

    async fn set_mapping(&self, mapping: Mapping) -> IndexResult<()> {
        self.write("recording a mapping", move |connection| {
            device_state::set_mapping(connection, &mapping)
        })
        .await
    }

    async fn mappings(&self) -> IndexResult<Vec<Mapping>> {
        self.read("reading the mappings", device_state::mappings)
            .await
    }

    async fn mark_present(&self, observation: LocalObservation) -> IndexResult<()> {
        self.write("recording a materialized file", move |connection| {
            device_state::mark_present(connection, &observation)
        })
        .await
    }

    async fn mark_absent(&self, path: &EntryPath, at: DeviceTime) -> IndexResult<()> {
        let path = path.clone();
        self.write("recording a file as gone", move |connection| {
            device_state::mark_absent(connection, &path, at)
        })
        .await
    }

    async fn local_entry_at(&self, path: &EntryPath) -> IndexResult<Option<LocalEntry>> {
        let path = path.clone();
        self.read("reading a local file's row", move |connection| {
            device_state::local_entry_at(connection, &path)
        })
        .await
    }

    async fn present_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<LocalEntry>> {
        let prefix = prefix.cloned();
        self.read("reading what this device has", move |connection| {
            device_state::present_under(connection, prefix.as_ref())
        })
        .await
    }

    async fn present_without_entry(&self) -> IndexResult<Vec<LocalEntry>> {
        self.read(
            "reading what the Library left behind",
            device_state::present_without_entry,
        )
        .await
    }

    async fn record_pending_upload(&self, pending: PendingUpload) -> IndexResult<()> {
        self.write("recording a spool", move |connection| {
            device_state::record_pending_upload(connection, &pending)
        })
        .await
    }

    async fn complete_pending_spool(&self, container_id: ContainerId) -> IndexResult<()> {
        self.write("completing a spool", move |connection| {
            device_state::complete_pending_spool(connection, container_id)
        })
        .await
    }

    async fn clear_pending_upload(&self, container_id: ContainerId) -> IndexResult<()> {
        self.write("clearing a spool", move |connection| {
            device_state::clear_pending_upload(connection, container_id)
        })
        .await
    }

    async fn pending_uploads(&self) -> IndexResult<Vec<PendingUpload>> {
        self.read("reading the spools", device_state::pending_uploads)
            .await
    }
}
