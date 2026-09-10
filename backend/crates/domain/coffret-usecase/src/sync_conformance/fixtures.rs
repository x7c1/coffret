use std::path::{Path, PathBuf};

use coffret_format::{generate_container_id, wrap_container_key, Purpose, PurposeKey};
use coffret_model::{
    Btime, ContainerAddition, ContainerId, ContainerKey, ContainerKind, ContainerSummary,
    ContentHash, EntryExtent, EntryMetadata, MasterKey, MasterKeyEpoch, Mtime,
};

use crate::byte_stream::ByteStream;
use crate::ciphertext_len_claims::ciphertext_len;
use crate::commit::{commit_batch, CommitPolicy, CommitRequest, PreparedAddition, PreparedBatch};
use crate::device_state::{
    BatchId, DeviceTime, LocalObservation, Mapping, PendingUpload, RootIdentity,
};
use crate::entry_paths::entry_path;
use crate::in_memory_fs::InMemoryFs;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync::{LibraryKeys, SyncRequest};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

// What the cases are built out of. The keys are real, derived from one real
// Master Key, because every case reads what it committed back off Storage the
// way another device would — a fixture that faked the crypto would prove
// nothing about what that device would find.

/// The Master Key the whole suite works under.
pub(super) fn master_key() -> MasterKey {
    MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN])
}

/// Everything a sync of the Library's first epoch seals with.
pub(super) fn keys() -> LibraryKeys {
    LibraryKeys::derive(&master_key(), MasterKeyEpoch::FIRST)
}

/// The purpose key one kind of object is sealed under (spec: KD-4).
///
/// The cases derive their own rather than borrowing the run's, so that what
/// they open a stored object with is the rule and not the code under test.
pub(super) fn purpose_key(purpose: Purpose) -> PurposeKey {
    PurposeKey::derive(&master_key(), purpose)
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

/// The clock the suite's `run`th sync of a case runs at.
///
/// Fixed rather than the real one, so that what a case writes into the device's
/// bookkeeping is the same on every machine.
pub(super) fn at(run: i64) -> DeviceTime {
    DeviceTime::from_unix_seconds(1_700_000_000 + run)
}

/// Where every suite's fixture spools, inside the in-memory filesystem it owns.
///
/// Any path at all, because nothing here is on a disk — what makes it a
/// directory is that the run prepares it. Fixed rather than per case so that a
/// case naming a spool file by hand spells the same path a run would.
///
/// The freeze and fetch suites borrow it, for the reason they borrow the rest of
/// what the spool means: all three drive the same two flows.
pub(crate) fn spool_dir() -> &'static Path {
    Path::new("/spool")
}

/// The mapped folder of every suite's fixture, in that same filesystem.
///
/// Fixed rather than the backend's, because there is nothing left for a backend
/// to choose: the folder is in the fake, and a case that wants two local roots
/// side by side makes subdirectories of this one. Deliberately not under the
/// spool directory, so that a run listing one never meets the other.
pub(crate) fn folder() -> &'static Path {
    Path::new("/folder")
}

/// One sync run against a store, a catalog, and the device's disk.
///
/// The disk is handed to both halves of the request, because it is one disk: the
/// spool it writes and the mapped folders it reads are two places in the same
/// fake, the way they are two places on a device.
///
/// The store travels separately from the fixture because one case runs against
/// a wrapper around it.
pub(super) fn request<'a>(
    store: &'a dyn ObjectStore,
    index: &'a dyn Index,
    keys: &'a LibraryKeys,
    fs: &'a InMemoryFs,
    run: i64,
) -> SyncRequest<'a> {
    SyncRequest::new(
        store,
        index,
        keys,
        fs,
        fs,
        spool_dir(),
        BatchId::new(format!("run-{run}")),
        at(run),
    )
    .with_policy(policy())
}

/// Maps the case's folder onto the Library at `prefix` (spec: EP-9).
pub(super) async fn map(fixture: &SyncUnderTest, prefix: Option<&str>) {
    map_at(fixture, prefix, fixture.folder()).await;
}

/// Maps a directory the case names, rather than the whole fixture folder
/// (spec: EP-9).
///
/// The fixture hands over one folder, so a case that needs two local roots side
/// by side — or one it can remove without taking the fixture's own directory with
/// it — maps subdirectories of it.
pub(super) async fn map_at(fixture: &SyncUnderTest, prefix: Option<&str>, local_root: &Path) {
    map_with(fixture, prefix, local_root, None).await;
}

/// The same, with a filesystem identity already recorded for the root
/// (spec: EP-12).
pub(super) async fn map_with(
    fixture: &SyncUnderTest,
    prefix: Option<&str>,
    local_root: &Path,
    root_identity: Option<RootIdentity>,
) {
    let mapping = Mapping::new(prefix.map(entry_path), local_root.to_path_buf());
    fixture
        .index()
        .set_mapping(match root_identity {
            Some(identity) => mapping.stamped(identity),
            None => mapping,
        })
        .await
        .expect("recording a mapping must succeed");
}

/// An identity no filesystem this case runs on has (spec: EP-12).
///
/// A mapping recorded with it is, to the comparison the guard makes, exactly
/// what an unmounted disk leaves behind: the recorded identity is not the one the
/// root stands on now. The real shape it stands for is a mount point whose device
/// was unplugged, so what is left at the path is an empty directory on the root
/// filesystem — a different device number from the one the mounted disk carried.
/// Arranging it this way needs no mount and no privileges.
///
/// Deliberately spelled the way no platform spells one, so that it cannot come
/// to equal what any real device or the fake answers with: an identity carries a
/// tag for the form it came from precisely so that two platforms' spellings can
/// never collide, and a fixture's stand-in is a third form again.
pub(crate) fn another_filesystem() -> RootIdentity {
    RootIdentity::new("a-filesystem-this-device-is-not-on")
}

/// Every mapping the device holds, so a case can assert what a run stamped
/// (spec: EP-12).
pub(super) async fn mappings(index: &dyn Index) -> Vec<Mapping> {
    index
        .mappings()
        .await
        .expect("asking the Index for its mappings must succeed")
}

/// Writes a file under a folder of the device's disk, making the folders above
/// it.
pub(crate) fn write(fs: &InMemoryFs, folder: &Path, relative: &str, content: &[u8]) -> PathBuf {
    let path = folder.join(relative);
    fs.write_file(&path, content);
    path
}

/// Moves a file's modification time without touching a byte of it.
///
/// Set outright rather than by rewriting the file: what EP-10's cheap comparison
/// reads is the length and the modification time, and the case that needs this is
/// exactly the one where nothing but that time may differ.
pub(crate) fn touch(fs: &InMemoryFs, path: &Path, seconds: i64) {
    fs.set_mtime(path, seconds);
}

/// What the device's disk says about a local file now.
pub(crate) fn observed(fs: &InMemoryFs, path: &Path) -> (u64, Mtime) {
    fs.observed(path)
        .expect("a case asks this of a file it planted")
}

/// What the device's disk says a local file was created at, if anything
/// (spec: FM-9).
///
/// Read off the disk rather than taken from the run, so that what a case
/// compares a committed Entry against is the filesystem's own answer and not the
/// code that captured it. `None` is a real answer: a filesystem that keeps no
/// birth time — a tmpfs, an older platform — is exactly the case an absent field
/// stands for, and it is checked as such rather than skipped.
///
/// Visible to the freeze suite, which asks the same question of the same disk:
/// what a walk captured is one account, not one per suite.
pub(crate) fn born(fs: &InMemoryFs, path: &Path) -> Option<Btime> {
    fs.born(path)
}

/// Commits a Container of the suite's own making.
///
/// Some cases need a Library state no sync produces — an Entry inside a Pack,
/// or one this device never materialized — so they commit it the way another
/// device would have. The object at the Container's name is not a real
/// Container and deliberately so: a sync never opens the Container an Entry
/// already lives in, so a case that needed real ciphertext there would be
/// asserting something the flow is not allowed to do.
#[allow(clippy::too_many_arguments)]
pub(super) async fn plant(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &LibraryKeys,
    kind: ContainerKind,
    path: &str,
    content: &[u8],
    mtime: Mtime,
    materialized: bool,
) -> ContainerId {
    let container_id = generate_container_id().expect("the OS CSPRNG is available");
    store
        .put(
            &container_id.object_name(),
            ByteStream::from(format!("ciphertext of {container_id}").into_bytes()),
        )
        .await
        .expect("storing a Container must succeed");

    let entry = EntryMetadata {
        path: entry_path(path),
        extent: EntryExtent::from_start(content.len() as u64)
            .expect("a fixture's content is shorter than the address space the format admits"),
        mtime,
        // A planted Container stands for one another device wrote, and nothing
        // says that device's platform reported a birth time (spec: FM-9).
        btime: None,
        hash: ContentHash::from_bytes(*blake3::hash(content).as_bytes()),
        derived_from: None,
        mime: None,
    };
    let envelope = wrap_container_key(
        &purpose_key(Purpose::ContainerWrap),
        &container_id,
        &ContainerKey::from_bytes([0x11; ContainerKey::BYTE_LEN]),
    )
    .expect("wrapping a Container Key must succeed");

    let mut batch = PreparedBatch::adding(vec![PreparedAddition::new(
        ContainerAddition::new(
            ContainerSummary {
                id: container_id,
                kind,
                ciphertext_hash: ContentHash::from_bytes([0x22; ContentHash::BYTE_LEN]),
                ciphertext_len: ciphertext_len(64),
                object_ref: None,
            },
            vec![entry],
        )
        .expect("a fixture holds a table that tiles its Container's stream"),
        envelope,
    )]);
    if materialized {
        batch = batch.materializing(vec![LocalObservation {
            path: entry_path(path),
            size: content.len() as u64,
            mtime,
            at: at(0),
        }]);
    }

    commit_batch(CommitRequest::new(store, index, keys.control(), batch).with_policy(policy()))
        .await
        .expect("committing a planted Container must succeed");
    container_id
}

/// Every Container a device is about to spool, has spooled, or has uploaded and
/// has not settled (spec: OC-2).
pub(super) async fn pending(index: &dyn Index) -> Vec<PendingUpload> {
    index
        .pending_uploads()
        .await
        .expect("asking the Index for pending uploads must succeed")
}

/// How many files the spool directory holds.
///
/// Read off the fake's own bookkeeping rather than through the capability, which
/// is also the one thing no flow does: the pending rows are the only handle on
/// what is in the spool, so a run that left a file no row names would be caught
/// here and by nothing else (spec: OC-2).
pub(crate) fn spooled(fs: &InMemoryFs) -> usize {
    fs.files_under(spool_dir()).len()
}

/// A moment in the past to restamp a file with.
pub(crate) const OLDER: i64 = 1_600_000_000;

/// A moment further along, for the touch that changes only the stamp.
pub(crate) const NEWER: i64 = 1_600_000_600;
