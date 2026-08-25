use std::path::{Path, PathBuf};

use coffret_format::{ContainerFootprint, DecodedContainer, EntryPlan};
use coffret_model::{
    ContainerId, ContainerKind, ContentHash, EntryPath, JournalRecord, MasterKey, MasterKeyEpoch,
};

use crate::commit::CommitPolicy;
use crate::conformance_library::Library;
use crate::device_state::{BatchId, DeviceTime, Mapping};
use crate::freeze::{freeze_folder, FreezeOutcome, FreezeRequest, LibraryKeys};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync::{sync_folders, SyncOutcome, SyncRequest};

// What the cases are built out of. The keys are real, derived from one real
// Master Key, because every case reads what it committed back off Storage the
// way another device would — a fixture that faked the crypto would prove nothing
// about what that device would find.

/// The Master Key both devices are enrolled under.
///
/// The same one the sync and fetch suites work under, so that a Library one
/// suite's helper builds is one another's helper can open.
pub(super) fn master_key() -> MasterKey {
    MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN])
}

/// Everything one epoch's Containers are sealed and opened with.
pub(super) fn keys() -> LibraryKeys {
    LibraryKeys::derive(&master_key(), MasterKeyEpoch::FIRST)
}

/// A policy that keeps a case's Library small and its checkpoints out of the
/// way.
pub(super) fn policy() -> CommitPolicy {
    CommitPolicy::default()
        .with_replica_count(2)
        .with_checkpoint_threshold(NEVER_CHECKPOINT)
}

/// A threshold no case reaches by committing.
const NEVER_CHECKPOINT: u64 = 1_000;

/// The clock the suite's `run`th operation runs at.
///
/// Fixed rather than the real one, so that what a case writes into a device's
/// bookkeeping is the same on every machine.
pub(super) fn at(run: i64) -> DeviceTime {
    DeviceTime::from_unix_seconds(1_700_000_000 + run)
}

/// A size target small enough that a handful of short files reaches it.
///
/// The production value is a measurement question about a Library of real
/// photographs (spec: PK-5); what a case needs is only that the boundary is
/// crossed several times over, which is cheaper to arrange by shrinking the
/// target than by writing gigabytes.
pub(super) const TARGET: u64 = 400;

/// A target roomy enough that several Entries share a Pack.
///
/// The cases about the cut want it to fall often; the case about reading a
/// folder back wants Packs that actually hold a few files, because what it is
/// really asserting is that a folder costs fewer fetches than it has files
/// (spec: PK-16).
pub(super) const ROOMY_TARGET: u64 = 800;

/// One freeze run over the source device's folder.
///
/// The store travels separately from the fixture because some cases run against
/// a wrapper around it.
pub(super) fn request<'a>(
    store: &'a dyn ObjectStore,
    index: &'a dyn Index,
    keys: &'a LibraryKeys,
    spool: &Path,
    target: u64,
    run: i64,
) -> FreezeRequest<'a> {
    FreezeRequest::new(
        store,
        index,
        keys,
        spool,
        target,
        BatchId::new(format!("freeze-{run}")),
        at(run),
    )
    .with_policy(policy())
}

/// Freezes the source device's folder, which the case expects to succeed.
pub(super) async fn freeze(
    fixture: &FreezeUnderTest,
    keys: &LibraryKeys,
    target: u64,
    run: i64,
) -> FreezeOutcome {
    freeze_against(fixture.store(), fixture, keys, target, run).await
}

/// The same, against a store the case wraps around the backend's.
pub(super) async fn freeze_against(
    store: &dyn ObjectStore,
    fixture: &FreezeUnderTest,
    keys: &LibraryKeys,
    target: u64,
    run: i64,
) -> FreezeOutcome {
    freeze_folder(request(
        store,
        fixture.source(),
        keys,
        fixture.spool(),
        target,
        run,
    ))
    .await
    .unwrap_or_else(|error| panic!("a freeze of the source folder must succeed: {error}"))
}

/// Freezes one folder of the source device's Library rather than all of it.
pub(super) async fn freeze_under(
    fixture: &FreezeUnderTest,
    keys: &LibraryKeys,
    prefix: &str,
    target: u64,
    run: i64,
) -> FreezeOutcome {
    freeze_folder(
        request(
            fixture.store(),
            fixture.source(),
            keys,
            fixture.spool(),
            target,
            run,
        )
        .under(EntryPath::new(prefix)),
    )
    .await
    .unwrap_or_else(|error| panic!("a narrowed freeze of the source folder must succeed: {error}"))
}

/// Carries the source device's folder into the Library one Container per file,
/// which is the state a freeze absorbs (spec: PK-1).
pub(super) async fn sync_source(
    fixture: &FreezeUnderTest,
    keys: &LibraryKeys,
    run: i64,
) -> SyncOutcome {
    sync_folders(
        SyncRequest::new(
            fixture.store(),
            fixture.source(),
            keys,
            fixture.spool(),
            BatchId::new(format!("sync-{run}")),
            at(run),
        )
        .with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("a sync of the source folder must succeed: {error}"))
}

/// Maps a device's folder onto the Library at `prefix` (spec: EP-9).
pub(super) async fn map(index: &dyn Index, prefix: Option<&str>, local_root: &Path) {
    index
        .set_mapping(Mapping {
            prefix: prefix.map(EntryPath::new),
            local_root: local_root.to_path_buf(),
        })
        .await
        .expect("recording a mapping must succeed");
}

/// Writes a file under a folder, making the directories above it.
pub(super) async fn write(folder: &Path, relative: &str, content: &[u8]) -> PathBuf {
    let path = folder.join(relative);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .expect("making a folder must succeed");
    }
    tokio::fs::write(&path, content)
        .await
        .expect("writing a file must succeed");
    path
}

/// One local file's whole content, which the case expects to be there.
pub(super) async fn read(path: &Path) -> Vec<u8> {
    tokio::fs::read(path)
        .await
        .unwrap_or_else(|error| panic!("reading a file must succeed: {error}"))
}

/// Content that differs in every byte, so a Pack that dropped or reordered
/// bytes lands on a different hash rather than on the same one.
pub(super) fn filler(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|index| (index as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

/// Opens one Container the long way round, as a second device would.
pub(super) async fn opened(
    store: &dyn ObjectStore,
    record: &JournalRecord,
    container_id: ContainerId,
) -> DecodedContainer {
    Library::read(store)
        .await
        .open(store, record, container_id, &master_key())
        .await
}

/// What a Pack of these Entries measures before padding (spec: PK-6).
///
/// The cases work it out from what a Container actually holds rather than from
/// what the run reported, so the assertion is about the Pack on Storage and not
/// about the accounting.
pub(super) fn footprint(container: &DecodedContainer) -> u64 {
    let plans: Vec<EntryPlan> = container
        .entries
        .iter()
        .map(|entry| EntryPlan::from(&entry.metadata))
        .collect();
    ContainerFootprint::of(ContainerKind::Pack, &plans)
        .expect("a Pack this size measures")
        .bytes()
}

/// What two Packs would measure merged, which is what PK-4's invariant is about.
pub(super) fn merged(left: &DecodedContainer, right: &DecodedContainer) -> u64 {
    let plans: Vec<EntryPlan> = left
        .entries
        .iter()
        .chain(&right.entries)
        .map(|entry| EntryPlan::from(&entry.metadata))
        .collect();
    ContainerFootprint::of(ContainerKind::Pack, &plans)
        .expect("a merged table measures")
        .bytes()
}

/// The BLAKE3-256 of some plaintext, which is what an entry table records.
pub(super) fn hash(content: &[u8]) -> ContentHash {
    ContentHash::from_bytes(*blake3::hash(content).as_bytes())
}

/// The handle Storage names one Container's object by (spec: FM-3).
///
/// The fetch suite already had to answer this — a store that mints identifiers
/// does not name objects by their names — so the answer is borrowed rather than
/// written a second time.
pub(super) use crate::fetch_conformance::fixtures::container_handle;

/// Commits a Keyring generation recording one Container's key as lost
/// (spec: KL-7, RV-8).
///
/// Borrowed from the fetch suite for the same reason: no flow produces this
/// state, losing a key is not something a commit does, and one hand-written
/// account of what a rebuild leaves behind is enough.
pub(super) use crate::fetch_conformance::fixtures::lose_key;

/// A moment in the past to stamp a file with.
pub(super) const OLDER: i64 = 1_600_000_000;

/// Moves a file's modification time without touching a byte of it.
pub(super) fn touch(path: &Path, seconds: i64) {
    use std::fs::FileTimes;
    use std::time::{Duration, UNIX_EPOCH};

    let file = std::fs::File::options()
        .write(true)
        .open(path)
        .expect("opening a file to restamp it must succeed");
    file.set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(seconds as u64)))
        .expect("setting a modification time must succeed");
}

/// How many files the spool directory holds.
pub(super) async fn spooled(spool: &Path) -> usize {
    let mut listing = match tokio::fs::read_dir(spool).await {
        Ok(listing) => listing,
        // A run that spooled nothing may never have made the directory. Any
        // other answer is a broken case rather than an empty spool.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return 0,
        Err(error) => panic!("listing the spool directory must succeed: {error}"),
    };
    let mut count = 0;
    while let Some(_entry) = listing
        .next_entry()
        .await
        .expect("listing the spool directory must succeed")
    {
        count += 1;
    }
    count
}
