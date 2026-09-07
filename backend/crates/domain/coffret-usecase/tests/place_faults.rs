//! What a fetch leaves behind when the disk under a mapped folder refuses.
//!
//! Not part of the fetch conformance suite, and deliberately: what these cases
//! arrange is a filesystem that fails at a chosen step, which the in-memory
//! destinations can be told to do and no real one can. They are about this
//! device rather than about a Storage backend, so they run against the
//! in-memory store alone.
//!
//! Every one of them asserts twice: the verdict the run returned, *and* the
//! state it left. The verdict is the smaller half. What EP-11's ordering is for
//! is that nothing a fetch has not fully verified ever appears at an Entry's own
//! name and nothing half-written is left inside a folder a later sync walks — so
//! what these cases really check is the folder afterwards: no scratch file, and
//! no file at the final name unless the run got as far as the rename.
//!
//! Two devices, each with a disk of its own. The source device carries a file
//! into the Library the ordinary way; the target device fetches it, and its disk
//! is the one that is scripted. Separate fakes because [`InMemoryFs::fail_on`]
//! counts per operation and not per capability: one disk under both would have a
//! case about the placement's first write counting the source device's spool
//! writes on the way there.
//!
//! [`InMemoryFs::fail_on`]: coffret_usecase::InMemoryFs::fail_on

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use coffret_logging::testing::CapturedLogs;
use coffret_model::{
    ContainerId, ContainerSummary, EntryLocation, EntryPath, IndexCheckpoint, JournalRecord,
    MasterKey, MasterKeyEpoch, SnapshotContent,
};
use coffret_usecase::commit::CommitPolicy;
use coffret_usecase::device_state::{
    BatchId, DeviceTime, LocalEntry, LocalObservation, Mapping, PendingUpload,
};
use coffret_usecase::fetch::{fetch_folders, FetchError, FetchOutcome, FetchRequest};
use coffret_usecase::sync::{sync_folders, SyncRequest};
use coffret_usecase::{
    CommittedBatch, InMemoryFs, InMemoryIndex, InMemoryStore, Index, IndexError, IndexResult,
    LibraryKeys, LocalOperation,
};
use tracing::Level;

/// Where a device spools, inside the in-memory filesystem it uses.
///
/// Any path at all: nothing here is on a disk, and what makes it a directory is
/// that the run prepares it.
const SPOOL_DIR: &str = "/spool";

/// Where a device's mapped folder stands in that same filesystem.
///
/// Any path at all, for the same reason — and deliberately not under the spool
/// directory, so that a run listing one never meets the other.
const FOLDER: &str = "/folder";

/// What the source device puts in the Library.
const HELD: &[u8] = b"what the Library holds";

/// A threshold no case reaches by committing.
const NEVER_CHECKPOINT: u64 = 1_000;

/// One device, and a disk that can be told to refuse.
struct Device {
    index: InMemoryIndex,
    fs: InMemoryFs,
}

impl Device {
    /// An empty catalog and a folder mapped onto the whole Library
    /// (spec: EP-9).
    async fn new() -> Self {
        let index = InMemoryIndex::new();
        index
            .set_mapping(Mapping {
                prefix: None,
                local_root: PathBuf::from(FOLDER),
                // No scan has seen this root yet, so nothing is recorded about
                // the filesystem under it (spec: EP-12).
                root_identity: None,
            })
            .await
            .expect("recording a mapping must succeed");

        let fs = InMemoryFs::new();
        fs.create_dir(Path::new(FOLDER));
        Self { index, fs }
    }

    /// Writes one file into the mapped folder.
    fn holding(self, relative: &str, content: &[u8]) -> Self {
        self.fs
            .write_file(&Path::new(FOLDER).join(relative), content);
        self
    }

    /// Every file in the mapped folder now, in path order.
    ///
    /// A temporary file a failed run left included, which is most of what these
    /// cases read it for: only the one that got as far as the rename has
    /// *placed* anything (spec: EP-11).
    fn files(&self) -> Vec<PathBuf> {
        self.fs.files_beneath(Path::new(FOLDER))
    }

    /// The local row this device wrote down about one Entry, if any
    /// (spec: EP-10).
    async fn local_row(&self, path: &str) -> Option<LocalEntry> {
        self.index
            .local_entry_at(&entry_path(path))
            .await
            .expect("asking the Index about a local path must succeed")
    }
}

/// A Library holding one file, carried in by a device of its own.
///
/// The source device is not what any case here is about — it is only how the
/// Library came to hold something — so it takes the ordinary path and its own
/// disk refuses nothing.
async fn library(path: &str) -> (InMemoryStore, Device) {
    let store = InMemoryStore::new(8);
    let source = Device::new().await.holding(path, HELD);
    sync_folders(
        SyncRequest::new(
            &store,
            &source.index,
            &keys(),
            &source.fs,
            &source.fs,
            SPOOL_DIR,
            BatchId::new("run-1"),
            at(1),
        )
        .with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("a sync of the source folder must succeed: {error}"));

    let target = Device::new().await;
    (store, target)
}

/// One fetch into the target device's folder, through its own disk.
async fn fetch(
    store: &InMemoryStore,
    index: &dyn Index,
    fs: &InMemoryFs,
) -> Result<FetchOutcome, FetchError> {
    let keys = keys();
    fetch_folders(FetchRequest::new(store, index, &keys, fs, at(2)).with_policy(policy())).await
}

/// Everything one epoch's Containers are sealed and opened with.
fn keys() -> LibraryKeys {
    LibraryKeys::derive(
        &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
        MasterKeyEpoch::FIRST,
    )
}

/// The clock a case's `run`th operation runs at.
fn at(run: i64) -> DeviceTime {
    DeviceTime::from_unix_seconds(1_700_000_000 + run)
}

/// A policy that keeps a case's Library small and its checkpoints out of the
/// way.
fn policy() -> CommitPolicy {
    CommitPolicy::default().with_checkpoint_threshold(NEVER_CHECKPOINT)
}

/// The Entry Path a literal spells, or a panic naming the one that spells none.
fn entry_path(text: &str) -> EntryPath {
    EntryPath::parse(text)
        .unwrap_or_else(|error| panic!("a fixture holds a literal Entry Path: {error}"))
}

/// The operation a run failed at, or a panic naming what it failed at instead.
fn refused_at(error: FetchError) -> LocalOperation {
    match error {
        FetchError::Io { operation, .. } => operation,
        other => panic!("a disk that refused must fail the run with Io, got {other:?}"),
    }
}

/// A temporary file that cannot be created stops the run before a byte is
/// written.
///
/// The near end of EP-11's window: the folder was reached and the file was never
/// made. There is nothing to clean up and nothing to undo — the catalog has not
/// been touched, so the next run simply asks again — and what the case is really
/// about is that the folder holds nothing at all afterwards, the final name
/// included.
#[tokio::test]
async fn a_scratch_that_cannot_be_created_fails_the_fetch_and_leaves_nothing_behind() {
    let (store, target) = library("a.jpg").await;
    target.fs.fail_on(LocalOperation::Creating, 1);

    let refused = fetch(&store, &target.index, &target.fs)
        .await
        .expect_err("the disk refused the temporary file");
    assert!(
        matches!(refused_at(refused), LocalOperation::Creating),
        "the run failed creating the temporary file, and says so",
    );

    assert!(
        target.files().is_empty(),
        "no temporary file was made, so the folder holds nothing (spec: EP-11)",
    );
    assert!(
        target.local_row("a.jpg").await.is_none(),
        "nothing was materialized, so there is nothing to write down (spec: EP-10)",
    );
}

/// A write that fails takes the temporary file with it.
///
/// The half-written file is the one thing a fetch must never leave inside a
/// folder a later sync walks: the scratch prefix already keeps a scan from
/// committing one, and removing it is what keeps the folder from accumulating
/// them. Nothing appears at the Entry's own name, because the rename is what
/// would have put it there.
#[tokio::test]
async fn a_write_that_fails_discards_the_scratch_and_fails_the_fetch() {
    let (store, target) = library("a.jpg").await;
    target.fs.fail_on(LocalOperation::Writing, 1);

    let refused = fetch(&store, &target.index, &target.fs)
        .await
        .expect_err("the disk refused the write");
    assert!(
        matches!(refused_at(refused), LocalOperation::Writing),
        "the run failed writing the plaintext, and says so",
    );

    assert!(
        target.files().is_empty(),
        "the temporary file is gone and nothing stands at the final name",
    );
    assert!(target.local_row("a.jpg").await.is_none());
}

/// A flush that fails leaves the same nothing, and for a sharper reason.
///
/// The bytes are all there by then and the run must still treat them as
/// worthless: what EP-11 makes the condition of a file becoming visible is that
/// its content is on the *device*, and a flush that refused is the disk saying it
/// may not be. So the temporary file goes and the final name stays empty, rather
/// than a rename publishing bytes a crash could still lose.
#[tokio::test]
async fn a_flush_that_fails_discards_the_scratch_and_fails_the_fetch() {
    let (store, target) = library("a.jpg").await;
    target.fs.fail_on(LocalOperation::Flushing, 1);

    let refused = fetch(&store, &target.index, &target.fs)
        .await
        .expect_err("the disk refused the flush");
    assert!(
        matches!(refused_at(refused), LocalOperation::Flushing),
        "the run failed flushing the file to the device, and says so",
    );

    assert!(
        target.files().is_empty(),
        "an unflushed file is not one to publish, so neither name holds anything",
    );
    assert!(target.local_row("a.jpg").await.is_none());
}

/// A stamp that fails leaves the same nothing.
///
/// The step between the flush and the rename: a placed file carries its Entry's
/// modification time and not the moment it was written (spec: FM-9, EP-11), and
/// a stamp that refused is the disk saying it does not. Publishing it anyway
/// would put a file at the Entry's own name with a time no scan can read as
/// what the Library holds, so the temporary file goes and the final name stays
/// empty, exactly as an unflushed one does.
#[tokio::test]
async fn a_stamp_that_fails_discards_the_scratch_and_fails_the_fetch() {
    let (store, target) = library("a.jpg").await;
    target.fs.fail_on(LocalOperation::Stamping, 1);

    let refused = fetch(&store, &target.index, &target.fs)
        .await
        .expect_err("the disk refused the stamp");
    assert!(
        matches!(refused_at(refused), LocalOperation::Stamping),
        "the run failed stamping the file with the Entry's time, and says so",
    );

    assert!(
        target.files().is_empty(),
        "an unstamped file is not one to publish, so neither name holds anything",
    );
    assert!(target.local_row("a.jpg").await.is_none());
}

/// A rename that fails takes the temporary file with it too.
///
/// The last moment before the file exists, and the one where a caller could most
/// easily be left holding something: the placement is verified, stamped, and
/// still invisible. Its own publish is what cleans up after itself here, because
/// the call consumes the placement and no caller is left with one to discard.
#[tokio::test]
async fn a_publish_that_fails_takes_the_scratch_with_it_and_fails_the_fetch() {
    let (store, target) = library("a.jpg").await;
    target.fs.fail_on(LocalOperation::Renaming, 1);

    let refused = fetch(&store, &target.index, &target.fs)
        .await
        .expect_err("the disk refused the rename");
    assert!(
        matches!(refused_at(refused), LocalOperation::Renaming),
        "the run failed renaming the file into place, and says so",
    );

    assert!(
        target.files().is_empty(),
        "neither the temporary file nor the final one is there",
    );
    assert!(
        target.local_row("a.jpg").await.is_none(),
        "nothing became visible, so nothing was written down (spec: EP-10)",
    );
}

/// A cleanup that fails after a failed write is recorded, and the write's
/// verdict is what the run reports.
///
/// Replacing the failure that made the cleanup necessary with "and the temporary
/// file would not go either" would lose the verdict a caller acts on. What is
/// left is a scratch file no run will come back for, which the scratch prefix
/// keeps a scan from reading as user data — so what is lost is tidiness rather
/// than correctness, and the record is the only account anybody has of it.
#[tokio::test]
async fn a_discard_that_fails_after_a_failed_write_is_logged_and_the_write_failure_is_reported() {
    let (store, target) = library("a.jpg").await;
    target.fs.fail_on(LocalOperation::Writing, 1);
    target.fs.fail_on(LocalOperation::Removing, 1);

    let logs = CapturedLogs::capture();
    let refused = fetch(&store, &target.index, &target.fs)
        .await
        .expect_err("the disk refused the write");
    assert!(
        matches!(refused_at(refused), LocalOperation::Writing),
        "the run reports the write that failed and not the cleanup after it",
    );

    let left = target.files();
    assert_eq!(
        left.len(),
        1,
        "the temporary file the cleanup could not remove is still there: {left:?}",
    );

    // Let go of, and never silently.
    let event = logs.only(Level::WARN);
    assert!(
        event
            .message()
            .contains("could not remove one of its own temporary files"),
        "{event}",
    );
    assert_eq!(
        event.field("error"),
        "Fetch::Io(operation=removed, kind=Other)",
        "which operation refused and what sort of refusal it was: {event}",
    );
    // And the file it was about is named by the operation rather than by its
    // path, which may never reach a log line (spec: EP-1).
    logs.assert_free_of(&[FOLDER, "a.jpg"]);
}

/// A catalog that refuses after the rename leaves the placed file where it is.
///
/// The one failure point past the moment the file exists. By then the bytes are
/// verified, stamped, and standing at the Entry's own name, so removing them
/// would be undoing content this device checked against the catalog on the
/// strength of a bookkeeping failure. The run fails — the device really has not
/// recorded the materialization, and the next one will ask again — and the file
/// stays (spec: EP-10, EP-11).
#[tokio::test]
async fn an_index_refusal_after_publish_leaves_the_placed_file_where_it_is() {
    let (store, target) = library("a.jpg").await;
    let refusing = RefusingIndex::around(&target.index);

    let refused = fetch(&store, &refusing, &target.fs)
        .await
        .expect_err("the catalog refused the materialization record");
    assert!(
        matches!(refused, FetchError::Index(_)),
        "a catalog that refused fails the run in the catalog's own words: {refused:?}",
    );

    assert_eq!(
        target.files(),
        vec![Path::new(FOLDER).join("a.jpg")],
        "the verified file stands at its final name, bookkeeping or no",
    );
    assert_eq!(
        target
            .fs
            .content(&Path::new(FOLDER).join("a.jpg"))
            .as_deref(),
        Some(HELD),
        "and it is the content the catalog names",
    );
    assert!(
        target.local_row("a.jpg").await.is_none(),
        "the row the run could not write is not there, so a later run asks again",
    );
}

/// A catalog that refuses the one write a placement makes, wrapped around the
/// real one.
///
/// [`Index::mark_present`] is the step whose failure leaves a file on disk that
/// the device has not written down, and that state cannot be reached by driving
/// the flow: the rename has to land and the record has to fail, in that order,
/// inside one call. Everything else is passed straight through, so what the case
/// reads back afterwards is the catalog the run really used.
struct RefusingIndex<'a> {
    inner: &'a dyn Index,
}

impl<'a> RefusingIndex<'a> {
    /// Refuses every [`mark_present`](Index::mark_present) and answers
    /// everything else honestly.
    fn around(inner: &'a dyn Index) -> Self {
        Self { inner }
    }
}

/// What the refused record reports.
///
/// A backend fault rather than anything about the observation: the catalog's own
/// store is what failed, which is the one shape of this failure that says
/// nothing is wrong with the file that was placed.
fn refused() -> IndexError {
    IndexError::Backend {
        operation: "mark_present",
        cause: Box::new(std::io::Error::other(
            "the catalog was interrupted after the file was renamed into place",
        )),
    }
}

#[async_trait]
impl Index for RefusingIndex<'_> {
    async fn restore(&self, snapshot: SnapshotContent) -> IndexResult<()> {
        self.inner.restore(snapshot).await
    }

    async fn apply(&self, record: JournalRecord) -> IndexResult<()> {
        self.inner.apply(record).await
    }

    async fn refresh(&self, batch: CommittedBatch) -> IndexResult<()> {
        self.inner.refresh(batch).await
    }

    async fn snapshot(&self) -> IndexResult<SnapshotContent> {
        self.inner.snapshot().await
    }

    async fn checkpoint(&self) -> IndexResult<Option<IndexCheckpoint>> {
        self.inner.checkpoint().await
    }

    async fn entry_at(&self, path: &EntryPath) -> IndexResult<Option<EntryLocation>> {
        self.inner.entry_at(path).await
    }

    async fn entries_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<EntryLocation>> {
        self.inner.entries_under(prefix).await
    }

    async fn containers_under(
        &self,
        prefix: Option<&EntryPath>,
    ) -> IndexResult<Vec<ContainerSummary>> {
        self.inner.containers_under(prefix).await
    }

    async fn set_mapping(&self, mapping: Mapping) -> IndexResult<()> {
        self.inner.set_mapping(mapping).await
    }

    async fn mappings(&self) -> IndexResult<Vec<Mapping>> {
        self.inner.mappings().await
    }

    async fn mark_present(&self, _observation: LocalObservation) -> IndexResult<()> {
        Err(refused())
    }

    async fn mark_absent(&self, path: &EntryPath, at: DeviceTime) -> IndexResult<()> {
        self.inner.mark_absent(path, at).await
    }

    async fn local_entry_at(&self, path: &EntryPath) -> IndexResult<Option<LocalEntry>> {
        self.inner.local_entry_at(path).await
    }

    async fn present_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<LocalEntry>> {
        self.inner.present_under(prefix).await
    }

    async fn present_without_entry(&self) -> IndexResult<Vec<LocalEntry>> {
        self.inner.present_without_entry().await
    }

    async fn record_pending_upload(&self, pending: PendingUpload) -> IndexResult<()> {
        self.inner.record_pending_upload(pending).await
    }

    async fn mark_spooled(&self, container_id: ContainerId) -> IndexResult<()> {
        self.inner.mark_spooled(container_id).await
    }

    async fn clear_pending_upload(&self, container_id: ContainerId) -> IndexResult<()> {
        self.inner.clear_pending_upload(container_id).await
    }

    async fn pending_uploads(&self) -> IndexResult<Vec<PendingUpload>> {
        self.inner.pending_uploads().await
    }
}
