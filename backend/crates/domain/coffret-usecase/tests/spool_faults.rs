//! What a sync and a freeze leave behind when the disk under the spool refuses.
//!
//! Not part of the sync or freeze conformance suites, and deliberately: what
//! these cases arrange is a filesystem that fails at a chosen step, which the
//! in-memory spool can be told to do and no real one can. They are about this
//! device rather than about a Storage backend, so they run against the in-memory
//! store alone.
//!
//! Every one of them asserts twice: the verdict the run returned, *and* the
//! state it left. The verdict is the smaller half. What OC-2's ordering is for is
//! that a run killed anywhere between announcing a spool and committing it leaves
//! a pending row that names what may be on the disk — so what these cases really
//! check is that the row is there, that it says what it should, that nothing
//! reached Storage, and that the next run settles it and carries the file in
//! exactly once.
//!
//! The mapped folder is in the same fake as the spool, because a device has one
//! disk. Nothing these cases arrange happens on the reading side of it — what
//! they script is the spool — but the folder is where it is for the same reason
//! the spool is: neither has to be a real directory for the run to be the real
//! run.

use std::path::{Path, PathBuf};

use coffret_logging::testing::CapturedLogs;
use coffret_model::{EntryPath, MasterKey, MasterKeyEpoch};
use coffret_usecase::commit::CommitPolicy;
use coffret_usecase::device_state::{BatchId, DeviceTime, Mapping, PendingUpload, SpoolState};
use coffret_usecase::freeze::{freeze_folder, FreezeError, FreezeOutcome, FreezeRequest};
use coffret_usecase::sync::{sync_folders, Reconciled, SyncError, SyncOutcome, SyncRequest};
use coffret_usecase::{
    InMemoryFs, InMemoryIndex, InMemoryStore, Index, LibraryKeys, LocalOperation, ObjectStore,
};
use tracing::Level;

/// Where the runs spool, inside the in-memory filesystem the device uses.
///
/// Any path at all: nothing here is on a disk, and what makes it a directory is
/// that the run prepares it.
const SPOOL_DIR: &str = "/spool";

/// Where the mapped folder stands in that same filesystem.
///
/// Any path at all, for the same reason — and deliberately not under the spool
/// directory, so that a run listing one never meets the other.
const FOLDER: &str = "/folder";

/// A threshold no case reaches by committing.
const NEVER_CHECKPOINT: u64 = 1_000;

/// A size target small enough that the two short files of a case make one Pack.
const PACK_TARGET: u64 = 4 * 1024;

/// One device, and a disk that can be told to refuse.
struct Device {
    store: InMemoryStore,
    index: InMemoryIndex,
    fs: InMemoryFs,
}

impl Device {
    /// An empty Library, an empty catalog, and a folder mapped onto the whole of
    /// it (spec: EP-9).
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
        Self {
            store: InMemoryStore::new(8),
            index,
            fs,
        }
    }

    /// Writes one file into the mapped folder.
    fn holding(self, relative: &str, content: &[u8]) -> Self {
        self.fs
            .write_file(&Path::new(FOLDER).join(relative), content);
        self
    }

    /// One sync run.
    async fn sync(&self, run: i64) -> Result<SyncOutcome, SyncError> {
        sync_folders(
            SyncRequest::new(
                &self.store,
                &self.index,
                &keys(),
                &self.fs,
                &self.fs,
                SPOOL_DIR,
                BatchId::new(format!("run-{run}")),
                at(run),
            )
            .with_policy(policy()),
        )
        .await
    }

    /// One freeze run over the whole folder.
    async fn freeze(&self, run: i64) -> Result<FreezeOutcome, FreezeError> {
        freeze_folder(
            FreezeRequest::new(
                &self.store,
                &self.index,
                &keys(),
                &self.fs,
                &self.fs,
                SPOOL_DIR,
                PACK_TARGET,
                BatchId::new(format!("freeze-{run}")),
                at(run),
            )
            .with_policy(policy()),
        )
        .await
    }

    /// The one pending row this device holds, and a panic where it holds any
    /// other number of them.
    async fn only_pending(&self) -> PendingUpload {
        let mut rows = self
            .index
            .pending_uploads()
            .await
            .expect("asking the Index for pending uploads must succeed");
        assert_eq!(
            rows.len(),
            1,
            "one spool was announced, so one row is open (spec: OC-2)",
        );
        rows.remove(0)
    }

    /// Every Container this device is still accounting for.
    async fn pending(&self) -> Vec<PendingUpload> {
        self.index
            .pending_uploads()
            .await
            .expect("asking the Index for pending uploads must succeed")
    }

    /// The spool files on the device's disk.
    fn spooled(&self) -> Vec<PathBuf> {
        self.fs.files_under(Path::new(SPOOL_DIR))
    }

    /// What Storage holds, which every case here expects to be nothing until
    /// something commits.
    async fn stored(&self) -> usize {
        self.store
            .list(None)
            .await
            .expect("listing the store must succeed")
            .objects
            .len()
    }
}

/// Everything one epoch's Containers are sealed with.
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

/// The operation a run failed at, or a panic naming what it failed at instead.
fn refused_at(error: SyncError) -> LocalOperation {
    match error {
        SyncError::Io { operation, .. } => operation,
        other => panic!("a disk that refused must fail the run with Io, got {other:?}"),
    }
}

/// A spool file that could not be created stops the run and leaves the row.
///
/// The far end of OC-2's window: the pending row is written and the file's
/// creation never happens. What the run leaves is a row naming a file that is not
/// there, which is a state disposal has to tolerate rather than be spared — and
/// nothing on Storage, because a Container is only ever uploaded out of a spool
/// its row calls finished.
///
/// The second half of the case is the convergence. The next run finds the row,
/// disposes of it, spools the source file again under a Container of its own, and
/// commits it once (spec: OC-2, OC-3).
#[tokio::test]
async fn a_spool_that_cannot_be_created_leaves_a_spooling_row_and_uploads_nothing() {
    let device = Device::new().await.holding("a.jpg", b"the file's bytes");
    device.fs.fail_on(LocalOperation::Creating, 1);

    let refused = device
        .sync(1)
        .await
        .expect_err("the disk refused the spool");
    assert!(
        matches!(refused_at(refused), LocalOperation::Creating),
        "the run failed at creating the spool file, and says so",
    );

    let row = device.only_pending().await;
    assert_eq!(
        row.state,
        SpoolState::Spooling,
        "the row was written before the file, so it never got past Spooling",
    );
    assert!(
        row.object_ref.is_none(),
        "a spool that was never created is never uploaded",
    );
    assert!(
        device.spooled().is_empty(),
        "the file the row names never came to exist",
    );
    assert_eq!(device.stored().await, 0, "nothing reached Storage");

    // And the run after it, over the state the first one left.
    let outcome = device
        .sync(2)
        .await
        .expect("a sync after a refused spool must succeed");
    assert_eq!(
        outcome.reconciled,
        vec![Reconciled::Disposed {
            container_id: row.container_id,
            // It never left the device, so there was nothing on Storage to
            // remove.
            trashed: false,
        }],
    );
    assert_eq!(
        outcome.added.len(),
        1,
        "the source file was spooled again, under a Container of its own",
    );
    assert_ne!(outcome.added[0], row.container_id);
    let commit = outcome.commit.expect("the file is worth a commit");
    assert_eq!(commit.record.additions().len(), 1, "one Entry, not two");
    assert!(
        device.pending().await.is_empty(),
        "a committed batch leaves no rows behind (spec: OC-2)",
    );
    assert!(device.spooled().is_empty());
}

/// A write into the spool that fails stops the run the same way.
///
/// What is at the path is then part of a Container, or nothing at all, and the
/// row says as much: a spool this device announced and never finished. Nothing
/// distinguishes it from the case above once the run is over, which is the point
/// — the row's state is the whole of what the next run needs, and it does not
/// have to ask the disk what happened.
#[tokio::test]
async fn a_spool_write_that_fails_leaves_a_spooling_row_and_uploads_nothing() {
    let device = Device::new().await.holding("a.jpg", b"the file's bytes");
    device.fs.fail_on(LocalOperation::Writing, 1);

    let refused = device
        .sync(1)
        .await
        .expect_err("the disk refused the write");
    assert!(
        matches!(refused_at(refused), LocalOperation::Writing),
        "the run failed writing the ciphertext, and says so",
    );

    let row = device.only_pending().await;
    assert_eq!(row.state, SpoolState::Spooling);
    assert!(row.object_ref.is_none());
    assert_eq!(device.stored().await, 0, "nothing reached Storage");

    let outcome = device
        .sync(2)
        .await
        .expect("a sync after a refused write must succeed");
    assert_eq!(
        outcome.reconciled,
        vec![Reconciled::Disposed {
            container_id: row.container_id,
            trashed: false,
        }],
    );
    assert_eq!(outcome.added.len(), 1);
    assert_ne!(outcome.added[0], row.container_id);
    assert!(device.pending().await.is_empty());
    assert!(device.spooled().is_empty());
}

/// A flush that fails leaves a whole Container on the disk and a row that does
/// not call it finished.
///
/// The one failure point where the bytes are all there and the run must still
/// treat them as worthless: the Container Key that opens them lived only in the
/// run that drew it, and the one place it would ever have been written down is
/// the Keyring the commit never reached (spec: KD-2, FM-14, KL-7). So the row
/// stays `Spooling`, nothing is uploaded, and the next run disposes of the file
/// rather than resuming it.
#[tokio::test]
async fn a_spool_flush_that_fails_leaves_a_spooling_row_and_uploads_nothing() {
    let device = Device::new().await.holding("a.jpg", b"the file's bytes");
    device.fs.fail_on(LocalOperation::Flushing, 1);

    let refused = device
        .sync(1)
        .await
        .expect_err("the disk refused the flush");
    assert!(
        matches!(refused_at(refused), LocalOperation::Flushing),
        "the run failed flushing the spool to the device, and says so",
    );

    let row = device.only_pending().await;
    assert_eq!(
        row.state,
        SpoolState::Spooling,
        "the run never got to say the file was whole",
    );
    assert!(row.object_ref.is_none());
    assert_eq!(
        device.spooled(),
        vec![row.spool_path.clone()],
        "the ciphertext the run did write is on the disk, and the row names it",
    );
    assert_eq!(device.stored().await, 0, "nothing reached Storage");

    let outcome = device
        .sync(2)
        .await
        .expect("a sync after a refused flush must succeed");
    assert_eq!(
        outcome.reconciled,
        vec![Reconciled::Disposed {
            container_id: row.container_id,
            trashed: false,
        }],
        "an unfinished spool is this device's own to reclaim (spec: OC-2)",
    );
    assert_eq!(outcome.added.len(), 1);
    assert_ne!(outcome.added[0], row.container_id);
    assert!(device.pending().await.is_empty());
    assert!(
        device.spooled().is_empty(),
        "neither the abandoned spool nor the committed one is still on the disk",
    );
}

/// A Pack whose flush fails leaves the same state, and the sync flow settles it.
///
/// The freeze writes its Container through the streaming encoder rather than in
/// one go (spec: PK-5), so this is a different write path reaching the same
/// promise — and what settles the row afterwards is
/// [`sync_folders`](coffret_usecase::sync::sync_folders), because an interrupted
/// run's spool is settled the same way whatever wrote it (spec: OC-2, OC-7).
#[tokio::test]
async fn a_pack_spool_flush_that_fails_leaves_a_spooling_row_and_uploads_nothing() {
    let device = Device::new()
        .await
        .holding("a.jpg", b"the first file's bytes")
        .holding("b.jpg", b"the second file's bytes");
    device.fs.fail_on(LocalOperation::Flushing, 1);

    let refused = device
        .freeze(1)
        .await
        .expect_err("the disk refused the flush");
    let FreezeError::Io { operation, .. } = refused else {
        panic!("a disk that refused must fail the run with Io, got {refused:?}");
    };
    assert!(
        matches!(operation, LocalOperation::Flushing),
        "the run failed flushing the Pack to the device, and says so",
    );

    let row = device.only_pending().await;
    assert_eq!(row.state, SpoolState::Spooling);
    assert!(row.object_ref.is_none());
    assert_eq!(
        device.spooled(),
        vec![row.spool_path.clone()],
        "the Pack the run did write is on the disk, and the row names it",
    );
    assert_eq!(device.stored().await, 0, "nothing reached Storage");

    let outcome = device
        .sync(2)
        .await
        .expect("a sync after a freeze that died mid-spool must succeed");
    assert_eq!(
        outcome.reconciled,
        vec![Reconciled::Disposed {
            container_id: row.container_id,
            trashed: false,
        }],
        "what a freeze left is settled by the sync flow, and disposed of",
    );
    assert_eq!(
        outcome.added.len(),
        2,
        "both files are carried in afresh, one Container each",
    );
    assert!(device.pending().await.is_empty());
    assert!(device.spooled().is_empty());
}

/// A spool that will not go after the commit landed does not fail the run.
///
/// The Library has already changed by then, and the commit's own refresh has
/// already dropped the pending rows, so a run that failed here would be
/// reporting a sync that did not happen. The removal is recorded and let go
/// instead; what is left is ciphertext no row names, which is orphan cleanup's to
/// find and a person's to decide on (spec: OC-1, OC-4).
#[tokio::test]
async fn a_discard_that_fails_after_a_commit_keeps_the_commit() {
    let device = Device::new().await.holding("a.jpg", b"the file's bytes");
    // The first removal of the run: the settling before the scan has no rows to
    // dispose of, so the post-commit cleanup is what this refuses.
    device.fs.fail_on(LocalOperation::Removing, 1);

    let logs = CapturedLogs::capture();
    let outcome = device
        .sync(1)
        .await
        .expect("a cleanup that failed after the commit must not fail the run");

    let commit = outcome.commit.expect("the file is worth a commit");
    assert_eq!(commit.record.additions().len(), 1);
    assert_eq!(outcome.added.len(), 1);
    assert!(
        device
            .index
            .entry_at(&EntryPath::parse("a.jpg").expect("a fixture holds a literal path"))
            .await
            .expect("asking the Index for a path must succeed")
            .is_some(),
        "the catalog holds the Entry the run committed",
    );
    assert!(
        device.pending().await.is_empty(),
        "the commit's refresh dropped the row, whatever became of the file",
    );
    assert_eq!(
        device.spooled().len(),
        1,
        "the spool that would not go is still on the disk, named by nothing",
    );

    // Let go of, and never silently. The record is the only account anybody has
    // of a file the rows no longer name, so a run that swallowed the refusal
    // outright would pass every assertion above and leave orphan cleanup with
    // nothing to go on.
    let event = logs.only(Level::WARN);
    assert!(
        event.message().contains("spool file could not be removed"),
        "{event}",
    );
    assert_eq!(
        event.field("reason"),
        "Local::Io(operation=removed, kind=Other)",
        "which operation refused and what sort of refusal it was: {event}",
    );
    // And the file it was about is named by the row's own vocabulary rather
    // than by its path, which may never reach a diagnostic event (spec: EL-1).
    logs.assert_free_of(&[SPOOL_DIR]);
}
