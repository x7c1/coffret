use crate::commit::CommitPolicy;
use crate::device_state::{BatchId, DeviceTime};
use crate::fetch::{FetchRequest, LibraryKeys};
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync::{sync_folders, SyncOutcome, SyncRequest};

/// A policy that keeps a case's Library small and its checkpoints out of the
/// way.
///
/// Two replicas rather than one, because one of the cases is about a fetch
/// stepping over a replica it cannot read (spec: RV-2), and that needs a second
/// position to step onto.
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
pub(crate) fn at(run: i64) -> DeviceTime {
    DeviceTime::from_unix_seconds(1_700_000_000 + run)
}

/// Carries the source device's folder into the Library (spec: CP-1).
pub(crate) async fn sync_source(
    fixture: &FetchUnderTest,
    keys: &LibraryKeys,
    run: i64,
) -> SyncOutcome {
    sync_folders(
        SyncRequest::new(
            fixture.store(),
            fixture.source(),
            keys,
            fixture.spool(),
            BatchId::new(format!("run-{run}")),
            at(run),
        )
        .with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("a sync of the source folder must succeed: {error}"))
}

/// One fetch run into the target device's folder.
///
/// The store travels separately from the fixture because some cases run against
/// a wrapper around it.
pub(crate) fn request<'a>(
    store: &'a dyn ObjectStore,
    index: &'a dyn Index,
    keys: &'a LibraryKeys,
    run: i64,
) -> FetchRequest<'a> {
    FetchRequest::new(store, index, keys, at(run)).with_policy(policy())
}
