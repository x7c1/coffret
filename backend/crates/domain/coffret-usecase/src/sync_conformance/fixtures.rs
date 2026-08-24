use std::fs::FileTimes;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use coffret_format::{generate_container_id, wrap_container_key, Purpose, PurposeKey};
use coffret_model::{
    ContainerAddition, ContainerId, ContainerKey, ContainerKind, ContainerSummary, ContentHash,
    EntryMetadata, EntryPath, MasterKey, MasterKeyEpoch, Mtime,
};

use crate::byte_stream::ByteStream;
use crate::commit::{commit_batch, CommitPolicy, CommitRequest, PreparedAddition, PreparedBatch};
use crate::device_state::{BatchId, DeviceTime, LocalObservation, Mapping, PendingUpload};
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync::{SyncKeys, SyncRequest};
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
pub(super) fn keys() -> SyncKeys {
    SyncKeys::derive(&master_key(), MasterKeyEpoch::FIRST)
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

/// One sync run against a store, a catalog, and a spool directory.
///
/// The store travels separately from the fixture because one case runs against
/// a wrapper around it.
pub(super) fn request<'a>(
    store: &'a dyn ObjectStore,
    index: &'a dyn Index,
    keys: &'a SyncKeys,
    spool: &Path,
    run: i64,
) -> SyncRequest<'a> {
    SyncRequest::new(
        store,
        index,
        keys,
        spool,
        BatchId::new(format!("run-{run}")),
        at(run),
    )
    .with_policy(policy())
}

/// Maps the case's folder onto the Library at `prefix` (spec: EP-9).
pub(super) async fn map(fixture: &SyncUnderTest, prefix: Option<&str>) {
    fixture
        .index()
        .set_mapping(Mapping {
            prefix: prefix.map(EntryPath::new),
            local_root: fixture.folder().to_path_buf(),
        })
        .await
        .expect("recording a mapping must succeed");
}

/// Writes a file under the case's folder, making the directories above it.
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

/// Moves a file's modification time without touching a byte of it.
///
/// Set outright rather than by rewriting the file: a rewrite within the same
/// second leaves the whole-second modification time where it was, and the case
/// that needs this is exactly the one where nothing but that time may differ.
pub(super) fn touch(path: &Path, seconds: u64) {
    let file = std::fs::File::options()
        .write(true)
        .open(path)
        .expect("opening a file to restamp it must succeed");
    file.set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(seconds)))
        .expect("setting a modification time must succeed");
}

/// What the filesystem says about a local file now.
pub(super) async fn observed(path: &Path) -> (u64, Mtime) {
    let metadata = tokio::fs::metadata(path)
        .await
        .expect("stating a file must succeed");
    let modified = metadata
        .modified()
        .expect("the filesystem keeps modification times")
        .duration_since(UNIX_EPOCH)
        .expect("the case's files are stamped after the epoch");
    (
        metadata.len(),
        Mtime::from_unix_seconds(modified.as_secs() as i64),
    )
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
    keys: &SyncKeys,
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
        path: EntryPath::new(path),
        offset: 0,
        size: content.len() as u64,
        mtime,
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
        ContainerAddition {
            container: ContainerSummary {
                id: container_id,
                kind,
                ciphertext_hash: ContentHash::from_bytes([0x22; ContentHash::BYTE_LEN]),
                ciphertext_len: 64,
                object_ref: None,
            },
            entries: vec![entry],
        },
        envelope,
    )]);
    if materialized {
        batch = batch.materializing(vec![LocalObservation {
            path: EntryPath::new(path),
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

/// Every Container this device has spooled and not settled (spec: OC-2).
pub(super) async fn pending(index: &dyn Index) -> Vec<PendingUpload> {
    index
        .pending_uploads()
        .await
        .expect("asking the Index for pending uploads must succeed")
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

/// A moment in the past to restamp a file with.
pub(super) const OLDER: u64 = 1_600_000_000;

/// A moment further along, for the touch that changes only the stamp.
pub(super) const NEWER: u64 = 1_600_000_600;
