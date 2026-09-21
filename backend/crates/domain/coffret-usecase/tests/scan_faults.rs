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

use coffret_logging::testing::CapturedLogs;
use coffret_model::{
    ControlObjectName, KeyringCommitment, MasterKey, MasterKeyEpoch, ObjectRef, ReplicaPosition,
};
use coffret_usecase::commit::CommitPolicy;
use coffret_usecase::device_state::{BatchId, DeviceTime, Mapping, PendingUpload, SpoolState};
use coffret_usecase::freeze::{freeze_folder, FreezeError, FreezeOutcome, FreezeRequest};
use coffret_usecase::sync::{sync_folders, Reconciled, SyncError, SyncOutcome, SyncRequest};
use coffret_usecase::{
    InMemoryFs, InMemoryIndex, InMemoryStore, Index, LibraryKeys, LocalOperation, ObjectStore,
};
use tracing::Level;

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

/// A freeze stopped before its commit still says the committed Keyring was
/// short (spec: KL-5, KL-15).
///
/// The finding is the read's, and the read happens first thing — before the
/// scan, before anything is packed, and long before a batch reaches the commit.
/// A run that gets as far as committing has the fuller account of the same set,
/// because the examination walks every position and says what it put back
/// (spec: KL-11, KL-15); this run reaches none of that, because the folder
/// refuses to list and the freeze leaves by the scan. What a person reads
/// afterwards is a run that failed for one reason and a Library that is a
/// replica short for another, and the second of the two is said here or nowhere.
///
/// The replica taken is position zero, because the walk stops at the first one
/// that answers: a set missing a position above the one that answered is a set
/// this read never looks at.
#[tokio::test]
async fn a_freeze_stopped_before_the_commit_still_reports_a_degraded_keyring() {
    let device = Device::new().await.holding("a.jpg", b"the file's bytes");
    let committed = device
        .freeze(1)
        .await
        .expect("a freeze over a folder of new files must succeed")
        .commit
        .expect("the file is worth a commit")
        .record
        .keyring()
        .clone();
    lose_replica(&device.store, &committed, 0).await;

    // The second listing this disk is asked for, and the first of the second
    // run: the freeze above listed the mapped root once, and a root is stated
    // rather than listed before its walk begins. The committed Keyring is read
    // before either, which is the whole point of the case.
    device.fs.fail_on(LocalOperation::Listing, 2);

    let logs = CapturedLogs::capture();
    let refused = device
        .freeze(2)
        .await
        .expect_err("the disk refused the listing");
    assert!(
        matches!(
            refused,
            FreezeError::Io {
                operation: LocalOperation::Listing,
                ..
            }
        ),
        "the run failed listing a mapped folder, and says so: {refused:?}",
    );

    let event = logs.only(Level::WARN);
    assert!(
        event
            .message()
            .contains("the committed Keyring is degraded"),
        "a run that reached no commit says what its read found: {event}",
    );
    assert_eq!(
        event.number("generation"),
        i64::try_from(committed.generation().get()).expect("a fixture commits one generation"),
        "and says which generation's set it was: {event}",
    );
    assert_eq!(
        event.number("stepped_over"),
        1,
        "one position was stepped over: {event}",
    );
    assert_eq!(
        event.number("replicas"),
        i64::from(committed.replica_count()),
        "out of the count the commitment declares: {event}",
    );
}

/// Takes one replica of a committed set out of Storage (spec: KL-5).
///
/// Through the recoverable removal, which is what object loss looks like from
/// the Library's side: the name leaves the listing, and a walk that goes
/// looking for it finds nothing there (spec: KL-1).
async fn lose_replica(store: &InMemoryStore, commitment: &KeyringCommitment, index: u16) {
    let replica = ReplicaPosition::new(index, commitment.replica_count())
        .expect("a declared replica index is a valid position");
    let name = ControlObjectName::keyring_replica(
        commitment.generation(),
        commitment.set_digest(),
        replica,
    )
    .expect("a committed digest is a valid one")
    .to_string();
    let handle = handle_of(store, &name).await;
    store
        .trash(&handle)
        .await
        .unwrap_or_else(|error| panic!("removing {name} must succeed: {}", every_link(&error)));
}

/// `error` and every link beneath it, joined the way a caller printing
/// `{error:#}` reads them.
///
/// A wrapper names only its own layer in `Display` and leaves what the layer
/// below answered to `source`, so a bare `{error}` in a panic drops everything
/// under the top line, which is usually the part that says what actually went
/// wrong.
///
/// A copy of the crate's own `error_chains::every_link` rather than a share of
/// it, and deliberately: this file is a test target of its own, compiled
/// against `coffret_usecase` as any other dependent would be, so a module the
/// crate keeps to itself is not something it can name. The alternative would be
/// to make the helper part of what the crate publishes, which is a wider
/// promise than a panic line in one file is worth.
fn every_link(error: &dyn std::error::Error) -> String {
    let mut rendered = error.to_string();
    let mut cause = error.source();
    while let Some(link) = cause {
        rendered.push_str(": ");
        rendered.push_str(&link.to_string());
        cause = link.source();
    }
    rendered
}

/// The handle Storage names one object by, which a case that wants that object
/// gone has to ask the listing for: a store that mints identifiers of its own
/// names nothing by the name it was stored under (spec: FM-3, FM-12).
async fn handle_of(store: &InMemoryStore, name: &str) -> ObjectRef {
    let mut token = None;
    loop {
        let page = store
            .list(token.as_ref())
            .await
            .expect("listing the store must succeed");
        if let Some(found) = page.objects.into_iter().find(|object| object.name == name) {
            return found.object_ref;
        }
        token = page.next;
        assert!(token.is_some(), "{name:?} must be in Storage");
    }
}
