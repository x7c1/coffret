use std::collections::BTreeSet;

use coffret_format::DecodedContainer;
use coffret_model::{
    ContainerId, EntryLocation, EntryMetadata, JournalRecord, MasterKey, MasterKeyEpoch,
};

use crate::commit::CommitPolicy;
use crate::conformance_library::Library;
use crate::delete::{delete_entries, DeleteOutcome, DeleteRequest, DeleteSelection};
use crate::delete_conformance::delete_under_test::DeleteUnderTest;
use crate::device_state::{BatchId, DeviceTime, Mapping};
use crate::entry_paths::entry_path;
use crate::freeze::{freeze_folder, FreezeOutcome, FreezeRequest, LibraryKeys};
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync::{sync_folders, SyncOutcome, SyncRequest};

/// The Master Key the device is enrolled under — the one every other suite
/// works under, so one suite's helpers open what another's wrote.
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
        .with_checkpoint_threshold(1_000)
}

/// The clock the suite's `run`th operation runs at.
fn at(run: i64) -> DeviceTime {
    DeviceTime::from_unix_seconds(1_700_000_000 + run)
}

/// A size target roomy enough that every file one freeze of a case packs
/// shares one Pack: what a case arranges is which Entries share a Pack, and
/// it arranges that by which invocation packed them (spec: PK-8).
const ROOMY: u64 = 64 * 1024;

/// Maps the device's folder onto the Library root (spec: EP-9).
pub(super) async fn map(fixture: &DeleteUnderTest) {
    fixture
        .index()
        .set_mapping(Mapping::new(None, fixture.folder().to_path_buf()))
        .await
        .expect("recording a mapping must succeed");
}

/// Writes one file into the device's folder, with a fixed modification time.
pub(super) fn write(fixture: &DeleteUnderTest, relative: &str, content: &[u8]) {
    let path =
        crate::sync_conformance::fixtures::write(fixture.fs(), fixture.folder(), relative, content);
    crate::sync_conformance::fixtures::touch(
        fixture.fs(),
        &path,
        crate::sync_conformance::fixtures::OLDER,
    );
}

/// Content that differs in every byte from one seed to the next.
pub(super) fn filler(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|index| (index as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

/// Packs every eligible file in the folder into one Pack, which the case
/// expects to happen (spec: PK-1, PK-7).
pub(super) async fn freeze(fixture: &DeleteUnderTest, run: i64) -> ContainerId {
    let outcome: FreezeOutcome = freeze_folder(
        FreezeRequest::new(
            fixture.store(),
            fixture.index(),
            &keys(),
            fixture.fs(),
            fixture.fs(),
            fixture.spool_dir(),
            ROOMY,
            BatchId::new(format!("freeze-{run}")),
            at(run),
        )
        .with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("a freeze of the folder must succeed: {error}"));
    assert_eq!(
        outcome.packs.len(),
        1,
        "a case's freeze packs what it wrote into one Pack",
    );
    outcome.packs[0].container_id
}

/// Carries the folder into the Library one Container per file (spec: PK-15).
pub(super) async fn sync(fixture: &DeleteUnderTest, run: i64) -> SyncOutcome {
    sync_on(fixture.index(), fixture, run).await
}

/// The same run, made by the device whose catalog is `index`.
pub(super) async fn sync_on(index: &dyn Index, fixture: &DeleteUnderTest, run: i64) -> SyncOutcome {
    sync_folders(
        SyncRequest::new(
            fixture.store(),
            index,
            &keys(),
            fixture.fs(),
            fixture.fs(),
            fixture.fs(),
            fixture.spool_dir(),
            BatchId::new(format!("sync-{run}")),
            at(run),
        )
        .with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("a sync of the folder must succeed: {error}"))
}

/// The request deleting `selection` from the device whose catalog is `index`,
/// against a store the case may wrap.
pub(super) fn request<'a>(
    store: &'a dyn ObjectStore,
    index: &'a dyn Index,
    fixture: &'a DeleteUnderTest,
    keys: &'a LibraryKeys,
    selection: DeleteSelection,
    run: i64,
) -> DeleteRequest<'a> {
    DeleteRequest::new(
        store,
        index,
        keys,
        fixture.fs(),
        fixture.spool_dir(),
        selection,
        BatchId::new(format!("delete-{run}")),
        at(run),
    )
    .with_policy(policy())
}

/// Deletes `selection`, which the case expects to succeed.
pub(super) async fn delete(
    store: &dyn ObjectStore,
    fixture: &DeleteUnderTest,
    selection: DeleteSelection,
    run: i64,
) -> DeleteOutcome {
    let keys = keys();
    delete_entries(request(
        store,
        fixture.index(),
        fixture,
        &keys,
        selection,
        run,
    ))
    .await
    .unwrap_or_else(|error| panic!("a deletion must succeed: {error}"))
}

/// A selection of exactly these Entry Paths.
pub(super) fn paths(paths: &[&str]) -> DeleteSelection {
    DeleteSelection::paths(paths.iter().map(|path| entry_path(*path)).collect())
}

/// A selection of one folder.
pub(super) fn folder(prefix: &str) -> DeleteSelection {
    DeleteSelection::folder(entry_path(prefix))
}

/// Where the current Entry at a path lives, if anywhere.
pub(super) async fn current(index: &dyn Index, path: &str) -> Option<EntryLocation> {
    index
        .entry_at(&entry_path(path))
        .await
        .expect("asking the catalog for a path must succeed")
}

/// One Container's rows, as the catalog records them, in stream order.
pub(super) async fn table(index: &dyn Index, container_id: ContainerId) -> Vec<EntryMetadata> {
    let mut rows: Vec<EntryMetadata> = index
        .entries_under(None)
        .await
        .expect("listing the catalog must succeed")
        .into_iter()
        .filter(|location| location.container_id == container_id)
        .map(|location| location.entry)
        .collect();
    rows.sort_by_key(|row| row.extent.range().start);
    rows
}

/// The Containers the catalog calls current.
pub(super) async fn current_containers(index: &dyn Index) -> BTreeSet<ContainerId> {
    index
        .containers_under(None)
        .await
        .expect("listing the catalog must succeed")
        .into_iter()
        .map(|summary| summary.id)
        .collect()
}

/// Opens one Container the long way round, as another device would.
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

/// Whether Storage still lists one Container's object — not trashed.
pub(super) async fn stored(store: &dyn ObjectStore, container_id: ContainerId) -> bool {
    Library::read(store).await.holds_container(container_id)
}

/// How many files the spool directory holds.
pub(super) use crate::sync_conformance::fixtures::spooled;

/// Commits a Keyring generation recording one Container's key as lost
/// (spec: KL-7), borrowed from the fetch suite for the reason the freeze suite
/// borrows it: no flow produces that state.
pub(super) use crate::fetch_conformance::fixtures::lose_key;

/// The handle Storage names one Container's object by (spec: FM-3).
pub(super) use crate::fetch_conformance::fixtures::container_handle;
