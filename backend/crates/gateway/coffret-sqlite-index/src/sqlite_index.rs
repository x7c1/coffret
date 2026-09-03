use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

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

/// How long a statement waits for another connection to let go of the file.
///
/// Seconds rather than milliseconds, because of what stands on either side of
/// the wait. What holds the write lock is one commit of one flow — a sync
/// recording what it uploaded, a fetch marking one file present — and what waits
/// is a listing somebody is looking at. A read that waited out a commit and then
/// answered is right; one that gave up after a fraction of a second and reported
/// the catalog unusable would be a failure invented out of two processes each
/// doing exactly what it is for.
const BUSY_TIMEOUT: Duration = Duration::from_secs(10);

/// One Library's [`Index`], kept in one SQLite file.
///
/// SQLite is synchronous and the port is not, so every operation runs on a
/// blocking thread against a single connection held under a lock. One connection
/// *per process* is enough: inside a process the lock is never the contended
/// thing, and a second connection would only bring SQLite's own writer
/// contention into a process that has no second writer.
///
/// A second *process* is another matter, and it is the ordinary arrangement
/// rather than a mistake: a server holding this catalog open to answer a browser
/// while the same person runs a sync in a terminal is two processes over one
/// file. So the file is opened in write-ahead logging mode, where readers and
/// one writer coexist and a read never waits on a write at all, and a write that
/// meets another process's write waits up to [`BUSY_TIMEOUT`] instead of
/// failing. Both settings are the file's and the connection's rather than this
/// type's, which is what makes them hold whichever process opened it.
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
    /// Nothing here is ever migrated: the catalog can be rebuilt from Storage,
    /// so discarding one written to a layout this build does not know is cheaper
    /// and safer than converting it (spec: RV-5). How much of the file the
    /// discard reaches depends on the layout it was written to. An older one
    /// whose device-local tables this build still reads keeps them — they are
    /// the device's own and nothing outside the file records them (spec: EP-9,
    /// EP-10, OC-2) — and loses only its catalog, which the next catch-up
    /// rebuilds. Anything else is refused with
    /// [`IndexError::UnsupportedSchema`], which says what the owner has to do
    /// with the file instead.
    ///
    /// The journal mode and the busy timeout are settled before the layout is
    /// looked at, because preparing the layout is itself a write and so is the
    /// first thing that could meet another process holding the file.
    pub fn open(path: impl AsRef<Path>) -> IndexResult<Self> {
        let mut connection = Connection::open(path).map_err(translate("opening the Index file"))?;
        share(&connection)?;
        schema::prepare(&mut connection)?;
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

/// Puts one connection into the arrangement two processes over one file need.
///
/// Write-ahead logging is a property of the *file* and survives it being closed,
/// so a second process finds it already set; the pragma is issued on every open
/// anyway, because whichever process gets there first is not decided anywhere.
/// The statement answers with the mode that is now in force, which is why it is
/// run as a query — SQLite reports a refusal by naming the mode it kept rather
/// than by failing, and a file on a filesystem with no shared memory keeps its
/// old one.
///
/// The busy timeout is the connection's own and is set on each of them.
fn share(connection: &Connection) -> IndexResult<()> {
    const OPERATION: &str = "preparing the Index file to be shared";

    connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        })
        .map_err(translate(OPERATION))?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(translate(OPERATION))
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

    async fn mark_spooled(&self, container_id: ContainerId) -> IndexResult<()> {
        self.write("marking a Container spooled", move |connection| {
            device_state::mark_spooled(connection, container_id)
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
