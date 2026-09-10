//! What a sync and a freeze leave behind when the disk under the mapped folders
//! refuses.
//!
//! The reading half of what `spool_faults` asks of the writing half, and here
//! for the same reason: what these cases arrange is a filesystem that fails at a
//! chosen step, which [`MappedRoots`](coffret_usecase::MappedRoots) can be told
//! to do and no real folder can. They are about this device rather than about a
//! Storage backend, so they run against the in-memory store alone.
//!
//! Every one of them asserts twice: the verdict the run returned, *and* the
//! state it left. Where the refusal lands decides which half matters. A scan
//! that stops before anything is announced must leave nothing at all — no
//! pending row, no spool file, nothing on Storage — because the ordering OC-2 is
//! written for has not begun yet. A read that fails partway through a Pack has
//! passed that line, so what it leaves is a row naming what may be on the disk,
//! and the case follows it to the run that settles it.
//!
//! The mapped folder and the spool are one in-memory filesystem, because a
//! device has one disk.

use std::path::{Path, PathBuf};

use coffret_model::{MasterKey, MasterKeyEpoch};
use coffret_usecase::commit::CommitPolicy;
use coffret_usecase::device_state::{BatchId, DeviceTime, Mapping, PendingUpload, SpoolState};
use coffret_usecase::freeze::{freeze_folder, FreezeError, FreezeOutcome, FreezeRequest};
use coffret_usecase::sync::{sync_folders, Reconciled, SyncError, SyncOutcome, SyncRequest};
use coffret_usecase::{
    InMemoryFs, InMemoryIndex, InMemoryStore, Index, LibraryKeys, LocalOperation, ObjectStore,
};

/// Where the runs spool, inside the in-memory filesystem the device uses.
const SPOOL_DIR: &str = "/spool";

/// Where the mapped folder stands in that same filesystem.
///
/// Deliberately not under the spool directory, so that a run listing one never
/// meets the other.
const FOLDER: &str = "/folder";

/// A threshold no case reaches by committing.
const NEVER_CHECKPOINT: u64 = 1_000;

/// A size target small enough that the short files of a case make one Pack.
const PACK_TARGET: u64 = 4 * 1024;

/// One device whose whole disk can be told to refuse.
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
            .set_mapping(Mapping::new(None, PathBuf::from(FOLDER)))
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
        let mut rows = self.pending().await;
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

/// Asserts that a refused run wrote nothing down anywhere.
///
/// The scan and the encode both happen before the pending row that opens OC-2's
/// window, so a run stopped in either of them has not begun the ordering at all:
/// no row names a file, no file is on the disk, and nothing has reached Storage.
/// A run that had got further would show itself here rather than in the verdict,
/// which is why every one of the three cases below ends with it.
async fn assert_nothing_written(device: &Device) {
    assert!(
        device.pending().await.is_empty(),
        "nothing was announced, so there is no row (spec: OC-2)",
    );
    assert!(
        device.spooled().is_empty(),
        "and no ciphertext was put on the disk",
    );
    assert_eq!(device.stored().await, 0, "nothing reached Storage");
}

/// A source file that will not open stops the sync before any spool exists.
///
/// The encode is where a sync reads a file it means to carry, and it happens
/// before the pending row that names the Container (spec: OC-2) — so a read
/// refused there leaves the device exactly as it was. The verdict says `Reading`
/// rather than something about the folder, because what was refused is the file
/// and not the walk that found it.
#[tokio::test]
async fn a_source_that_cannot_be_opened_fails_the_sync_before_any_spool_exists() {
    let device = Device::new().await.holding("a.jpg", b"the file's bytes");
    // The first read of the whole run: the scan finds the file new, so it hashes
    // nothing, and the encode's own read of it is what this refuses.
    device.fs.fail_on(LocalOperation::Reading, 1);

    let refused = device
        .sync(1)
        .await
        .expect_err("the disk refused the source file");
    assert!(
        matches!(refused_at(refused), LocalOperation::Reading),
        "the run failed opening the file it was about to encode, and says so",
    );

    assert_nothing_written(&device).await;
}

/// A mapped folder that will not list stops the sync and names the listing.
///
/// A folder that is *not there* is a verdict the walk has — the root is
/// unavailable, or a subfolder went away mid-walk — and a folder that is there
/// and refuses is not. Telling them apart is the capability's, so the run fails
/// with the operation the operating system refused rather than reading an empty
/// folder into an inference about deletions (spec: EP-12).
#[tokio::test]
async fn a_folder_that_cannot_be_listed_fails_the_sync_and_names_the_listing() {
    let device = Device::new().await.holding("a.jpg", b"the file's bytes");
    // The walk's listing of the mapped root, which is the run's first: the root
    // probe is a stat and not a listing, so nothing lists before it.
    device.fs.fail_on(LocalOperation::Listing, 1);

    let refused = device
        .sync(1)
        .await
        .expect_err("the disk refused the listing");
    assert!(
        matches!(refused_at(refused), LocalOperation::Listing),
        "the run failed listing a mapped folder, and says so",
    );

    assert_nothing_written(&device).await;
}

/// A mapped root that will not stat stops the sync and names the stat.
///
/// The root is stated before it is listed, so a root the process may not look at
/// fails at the call that was actually refused. It is the same shape as the
/// missing root and must not read as one: absence is the `None` the capability
/// answers with, and everything else is a refusal (spec: EP-12).
#[tokio::test]
async fn a_root_that_cannot_be_stated_fails_the_sync_and_names_the_stat() {
    let device = Device::new().await.holding("a.jpg", b"the file's bytes");
    device.fs.fail_on(LocalOperation::Stating, 1);

    let refused = device
        .sync(1)
        .await
        .expect_err("the disk refused the stat of the mapped root");
    assert!(
        matches!(refused_at(refused), LocalOperation::Stating),
        "the run failed stating a mapped root, and says so",
    );

    assert_nothing_written(&device).await;
}

/// A member that cannot be read while a Pack is being written leaves a
/// `Spooling` row and uploads nothing.
///
/// Past the line the three cases above stop short of. A Pack's members are read
/// twice — once by the scan, to hash them into the entry table, and once by the
/// spool step, which streams them through the encoder (spec: FM-2, FM-5, PK-5) —
/// and the pending row is written before the first byte of the Pack. So a read
/// refused in the *second* pass leaves ciphertext on the disk that a row names,
/// which is exactly the positive local provenance OC-2's ordering exists for.
///
/// A spool is never resumed — the Container Key that opens it lived only in the
/// run that drew it (spec: KD-2, FM-14) — so the next runs dispose of the row and
/// carry both files in afresh.
#[tokio::test]
async fn a_member_that_cannot_be_read_while_packing_leaves_a_spooling_row_and_uploads_nothing() {
    let device = Device::new()
        .await
        .holding("a.jpg", b"the first file's bytes")
        .holding("b.jpg", b"the second file's bytes");
    // The eleventh read of the run, counted the way the fake counts them: an
    // open and two reads per file for the scan's hashing (six), then the same
    // three for the Pack's first member (nine), then the second member's open
    // (ten). Eleven is that member's first content read — the Pack already holds
    // one whole file by then, so the refusal really does land mid-stream.
    device.fs.fail_on(LocalOperation::Reading, 11);

    let refused = device
        .freeze(1)
        .await
        .expect_err("the disk refused a member of the Pack");
    let FreezeError::Io { operation, .. } = refused else {
        panic!("a disk that refused must fail the run with Io, got {refused:?}");
    };
    assert!(
        matches!(operation, LocalOperation::Reading),
        "the run failed reading a file into the Pack, and says so",
    );

    let row = device.only_pending().await;
    assert_eq!(
        row.state,
        SpoolState::Spooling,
        "the run never got to say the Pack was whole (spec: OC-2)",
    );
    assert!(
        row.object_ref.is_none(),
        "a Pack that was never finished is never uploaded",
    );
    assert_eq!(
        device.spooled(),
        vec![row.spool_path.clone()],
        "what the run did write is on the disk, and the row names it",
    );
    assert_eq!(device.stored().await, 0, "nothing reached Storage");

    // And the runs after it, over the state the refused one left. The script is
    // spent — its count is long past — so these are ordinary runs.
    let settled = device
        .sync(2)
        .await
        .expect("a sync after a freeze that died mid-Pack must succeed");
    assert_eq!(
        settled.reconciled,
        vec![Reconciled::Disposed {
            container_id: row.container_id,
            // It never left the device, so there was nothing on Storage to
            // remove.
            trashed: false,
        }],
        "what a freeze left is settled by the sync flow, and disposed of",
    );
    assert_eq!(
        settled.added.len(),
        2,
        "both files are carried in afresh, one Container each",
    );

    let packed = device
        .freeze(3)
        .await
        .expect("a freeze over the settled folder must succeed");
    assert_eq!(packed.packs.len(), 1, "both files fit one Pack");
    assert_eq!(
        packed.frozen_entries(),
        2,
        "and every file is packed exactly once",
    );
    assert!(
        device.pending().await.is_empty(),
        "a committed batch leaves no rows behind (spec: OC-2)",
    );
    assert!(device.spooled().is_empty());
}
