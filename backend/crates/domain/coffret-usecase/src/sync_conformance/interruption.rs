use std::path::{Path, PathBuf};

use coffret_format::generate_container_id;
use coffret_model::{ContainerId, EntryPath, ObjectRef};

use crate::byte_stream::ByteStream;
use crate::conformance_library::Library;
use crate::device_state::{BatchId, PendingUpload, SpoolState};
use crate::index::Index;
use crate::index_error::IndexError;
use crate::object_store::ObjectStore;
use crate::sync::{sync_folders, Reconciled, SyncError};
use crate::sync_conformance::fixtures::{at, keys, map, pending, request, spooled, write};
use crate::sync_conformance::sync_under_test::SyncUnderTest;
use crate::sync_conformance::watching_index::WatchingIndex;

/// A run killed before it uploaded converges on one committed Entry.
///
/// What the earlier run left is a spool file and the row naming it, and neither
/// is resumable: the Container Key that opens that ciphertext lived only in the
/// run that drew it, and the one place it would ever have been written down is
/// the Keyring the commit never reached (spec: KD-2, FM-14, KL-7). So the run
/// disposes of both and spools the source file again, and what the Library ends
/// up with is one Entry — not two, and not none.
pub async fn a_spool_left_by_an_interrupted_run_converges_to_one_entry(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    write(fixture.folder(), "a.jpg", b"the file's bytes").await;
    let abandoned = interrupted(index, fixture.spool(), None).await;

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 2))
        .await
        .expect("a sync after an interrupted one must succeed");

    assert_eq!(
        outcome.added.len(),
        1,
        "the source file was spooled again, under a Container of its own",
    );
    assert_ne!(outcome.added[0], abandoned);
    let commit = outcome.commit.expect("the file is worth a commit");
    assert_eq!(commit.record.additions.len(), 1, "one Entry, not two");

    assert_eq!(
        outcome.reconciled,
        vec![Reconciled::Disposed {
            container_id: abandoned,
            // It never left the device, so there was nothing on Storage to
            // remove.
            trashed: false,
        }],
        "the abandoned spool was disposed of (spec: OC-2)",
    );

    assert_eq!(
        spooled(fixture.spool()).await,
        0,
        "neither the abandoned spool nor the committed one is still on disk",
    );
    assert!(pending(index).await.is_empty());
    assert!(index
        .entry_at(&EntryPath::nfc("a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .is_some(),);
}

/// A run killed after it uploaded converges too, and its object is disposed of.
///
/// The Container is on Storage and no Journal record names it, which is not by
/// itself proof of an orphan — Storage may be withholding a record (spec:
/// OC-1). What makes it disposable is this device's own row: it names the batch
/// that created the Container, and the caught-up Index says nothing makes it
/// current, which is the batch-was-abandoned proof (spec: OC-2, OC-3).
pub async fn an_uploaded_but_uncommitted_container_converges_to_one_entry(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    write(fixture.folder(), "a.jpg", b"the file's bytes").await;
    let abandoned = interrupted(index, fixture.spool(), Some(store)).await;
    assert!(
        Library::read(store).await.holds_container(abandoned),
        "the interrupted run got as far as uploading",
    );

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 2))
        .await
        .expect("a sync after an interrupted upload must succeed");

    assert_eq!(outcome.added.len(), 1);
    let commit = outcome.commit.expect("the file is worth a commit");
    assert_eq!(commit.record.additions.len(), 1, "one Entry, not two");
    assert!(
        !commit
            .record
            .additions
            .iter()
            .any(|addition| addition.container.id == abandoned),
        "the abandoned Container is not what the batch committed",
    );

    assert_eq!(
        outcome.reconciled,
        vec![Reconciled::Disposed {
            container_id: abandoned,
            trashed: true,
        }],
        "an uploaded Container no record names is moved out of the way",
    );
    assert!(
        !Library::read(store).await.holds_container(abandoned),
        "it leaves the listing, recoverably",
    );

    assert_eq!(spooled(fixture.spool()).await, 0);
    assert!(pending(index).await.is_empty());
}

/// A run with nothing to upload reads the head itself and settles the row.
///
/// Deciding that no record names a Container takes an Index that has read the
/// Library's head (spec: CK-9, OC-3), and a run with nothing to upload commits
/// nothing — so it reads the head itself, rather than leaving the object, its
/// spool, and the row to some later run that happens to have a file to carry.
/// And it settles against that head before the scan, because a row left open is
/// exactly what makes a scan read a path this device has already committed as
/// one it never materialized (spec: EP-10).
///
/// What is settled here is the abandoned half of the two verdicts: no record
/// names the Container, so its object goes to the trash and the local provenance
/// goes with it (spec: OC-2, OC-3).
pub async fn an_uploaded_container_is_settled_by_the_next_run(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    // Nothing in the folder, so this run has nothing to spool and no reason to
    // spend a generation.
    let abandoned = interrupted(index, fixture.spool(), Some(store)).await;

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 2))
        .await
        .expect("a sync with nothing to upload must succeed");

    assert!(
        outcome.commit.is_none(),
        "the folder held nothing to upload"
    );
    assert_eq!(
        outcome.reconciled,
        vec![Reconciled::Disposed {
            container_id: abandoned,
            trashed: true,
        }],
        "the run read the head itself rather than waiting for one that commits",
    );
    assert!(
        !Library::read(store).await.holds_container(abandoned),
        "no record names it, so it leaves the listing, recoverably",
    );
    assert!(
        pending(index).await.is_empty(),
        "the provenance goes with what it was provenance for (spec: OC-2)",
    );
    assert_eq!(spooled(fixture.spool()).await, 0);

    // And again over what the first run left, which is nothing to do rather
    // than something to fail at (spec: OC-6).
    let again = sync_folders(request(store, index, &keys, fixture.spool(), 3))
        .await
        .expect("running the settlement again must succeed");
    assert!(again.reconciled.is_empty());
}

/// A pending row whose spool is already gone is dropped rather than kept.
///
/// A row that says its spool was finished, over a file that has since left the
/// disk — half of a cleanup some earlier run got through. Nothing about it is
/// recoverable and nothing about it is on Storage, so the row is bookkeeping for
/// a Container that no longer exists anywhere. Dropping one that is already half
/// gone is idempotent, which is what lets an interrupted cleanup simply be run
/// again (spec: OC-6).
pub async fn a_stale_pending_row_is_dropped_with_its_spool(fixture: &SyncUnderTest) {
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let container_id = generate_container_id().expect("the OS CSPRNG is available");
    plant_row(
        index,
        container_id,
        fixture.spool().join("a-spool-that-is-not-there"),
        SpoolState::Spooled,
        None,
    )
    .await;

    let outcome = sync_folders(request(fixture.store(), index, &keys, fixture.spool(), 2))
        .await
        .expect("a sync over a stale row must succeed");

    assert!(
        outcome.commit.is_none(),
        "the folder held nothing to upload"
    );
    assert_eq!(
        outcome.reconciled,
        vec![Reconciled::Disposed {
            container_id,
            trashed: false,
        }],
    );
    assert!(pending(index).await.is_empty());

    // Again, over the state the first run left: the second finds nothing to do
    // rather than failing at what the first already did (spec: OC-6).
    let again = sync_folders(request(fixture.store(), index, &keys, fixture.spool(), 3))
        .await
        .expect("running the cleanup again must succeed");
    assert!(again.reconciled.is_empty());
}

/// Every spool file is named by a row before it can exist (spec: OC-2).
///
/// This is the ordering the whole of orphan cleanup over local ciphertext rests
/// on. A spool file that no row names is unreachable: nothing in the flow ever
/// lists the spool directory, so the row is the only handle on it there will ever
/// be, and a run that wrote the file first would leave one behind on every
/// interruption in between — a full disk, a source file that moved, a kill.
///
/// So the row goes first, and the catalog the run drives asserts it as each row
/// arrives: the path it names must not be there yet, and it may carry no object
/// handle. One such row per Container the run spooled is what says the ordering
/// held for all of them and not merely for the first.
pub async fn a_row_precedes_the_first_byte_of_a_spool(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    write(fixture.folder(), "a.jpg", b"the first file's bytes").await;
    write(fixture.folder(), "b/c.jpg", b"the second file's bytes").await;

    let watching = WatchingIndex::around(index);
    let outcome = sync_folders(request(store, &watching, &keys, fixture.spool(), 1))
        .await
        .expect("a watched sync must succeed");

    let commit = outcome.commit.expect("two new files are worth a commit");
    assert_eq!(
        commit.record.additions.len(),
        2,
        "both files were committed"
    );
    assert_eq!(outcome.added.len(), 2);
    assert_eq!(
        watching.spooling_rows(),
        outcome.added.len(),
        "every Container the run spooled was announced before its file existed",
    );
    assert!(
        pending(index).await.is_empty(),
        "a committed batch leaves no rows behind (spec: OC-2)",
    );
}

/// A spool the run never finished is disposed of, along with the row naming it.
///
/// The failure this rule exists for. The run stops between creating a spool file
/// and marking it `Spooled`, so what is at the path is ciphertext no row calls
/// whole — here a Container the run did finish writing and never got to mark,
/// elsewhere half of one or no bytes at all — and nothing on Storage or in the
/// Library will ever refer to it either way. The row says that much and no more: a
/// spool this device announced and never finished, with no object behind it.
///
/// Settling it takes no verdict from the Library. Nothing that was never
/// uploaded can be current, so the file and its row go whatever the Library
/// holds (spec: OC-2, OC-3), and the source file is spooled again into a
/// Container of its own — committed exactly once, and never under the abandoned
/// ID.
pub async fn an_unfinished_spool_is_disposed_with_its_row(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    write(fixture.folder(), "a.jpg", b"the file's bytes").await;

    let watching = WatchingIndex::refusing_to_mark_spooled(index);
    let result = sync_folders(request(store, &watching, &keys, fixture.spool(), 1)).await;
    let Err(SyncError::Index(IndexError::Backend { .. })) = &result else {
        panic!("a refused marking must fail the run that spooled, got {result:?}");
    };
    assert_eq!(
        watching.spooling_rows(),
        1,
        "the row was announced before the file the run then failed over",
    );

    let rows = pending(index).await;
    assert_eq!(rows.len(), 1, "one spool was announced, so one row is open");
    assert_eq!(
        rows[0].state,
        SpoolState::Spooling,
        "the run never got to say the file was whole",
    );
    assert!(
        rows[0].object_ref.is_none(),
        "an unfinished spool is never uploaded",
    );
    let abandoned = rows[0].container_id;
    assert_eq!(
        spooled(fixture.spool()).await,
        1,
        "the ciphertext the run did write is still on disk, and the row names it",
    );

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 2))
        .await
        .expect("a sync after an unfinished spool must succeed");

    assert_eq!(
        outcome.reconciled,
        vec![Reconciled::Disposed {
            container_id: abandoned,
            // It never left the device, so there was nothing on Storage to
            // remove.
            trashed: false,
        }],
        "an unfinished spool is this device's own to reclaim (spec: OC-2)",
    );
    assert_eq!(
        outcome.added.len(),
        1,
        "the source file was spooled again, under a Container of its own",
    );
    assert_ne!(outcome.added[0], abandoned);

    let commit = outcome.commit.expect("the file is worth a commit");
    assert_eq!(commit.record.additions.len(), 1, "one Entry, not two");
    assert!(
        !commit
            .record
            .additions
            .iter()
            .any(|addition| addition.container.id == abandoned),
        "the abandoned Container is not what the batch committed",
    );
    assert!(
        !commit.record.removals.contains(&abandoned),
        "a Container no record ever added is not one a record removes",
    );
    assert!(
        !Library::read(store).await.holds_container(abandoned),
        "the abandoned spool never reached Storage",
    );

    assert_eq!(
        spooled(fixture.spool()).await,
        0,
        "neither the abandoned spool nor the committed one is still on disk",
    );
    assert!(pending(index).await.is_empty());
}

/// A Spooling row whose spool file was never created is disposed of like any
/// other.
///
/// The far end of the same window: the row is written and the file's creation
/// never happens — a full disk, or a kill between the two calls. Disposal has to
/// treat a missing file as the outcome it wanted rather than as an error, or the
/// ordering that closes the window would open a state no run could get past.
///
/// The state is what separates this from
/// [`a_stale_pending_row_is_dropped_with_its_spool`]: both plant a row over a
/// file that is not on disk, and only the row's state says whether the file was
/// never created or was cleaned up after being finished.
///
/// And it settles in one run. A second finds nothing to do rather than failing at
/// what the first already did (spec: OC-6).
pub async fn a_spooling_row_whose_spool_was_never_created_is_disposed(fixture: &SyncUnderTest) {
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let container_id = generate_container_id().expect("the OS CSPRNG is available");
    plant_row(
        index,
        container_id,
        fixture.spool().join(format!("{container_id}.spool")),
        SpoolState::Spooling,
        None,
    )
    .await;

    let outcome = sync_folders(request(fixture.store(), index, &keys, fixture.spool(), 2))
        .await
        .expect("a sync over a row whose file never appeared must succeed");

    assert!(
        outcome.commit.is_none(),
        "the folder held nothing to upload"
    );
    assert_eq!(
        outcome.reconciled,
        vec![Reconciled::Disposed {
            container_id,
            trashed: false,
        }],
    );
    assert!(pending(index).await.is_empty());
    assert_eq!(spooled(fixture.spool()).await, 0);

    let again = sync_folders(request(fixture.store(), index, &keys, fixture.spool(), 3))
        .await
        .expect("running the cleanup again must succeed");
    assert!(again.reconciled.is_empty());
}

/// Leaves behind what a run killed mid-batch would have: a spool file, a row
/// naming it, and — where a store is given — the object it had already put up.
async fn interrupted(
    index: &dyn Index,
    spool: &Path,
    store: Option<&dyn ObjectStore>,
) -> ContainerId {
    let container_id = generate_container_id().expect("the OS CSPRNG is available");
    let ciphertext = format!("ciphertext of {container_id}").into_bytes();

    tokio::fs::create_dir_all(spool)
        .await
        .expect("making the spool directory must succeed");
    let spool_path: PathBuf = spool.join(format!("{container_id}.spool"));
    tokio::fs::write(&spool_path, &ciphertext)
        .await
        .expect("writing a spool file must succeed");

    let object_ref = match store {
        Some(store) => Some(
            store
                .put(&container_id.object_name(), ByteStream::from(ciphertext))
                .await
                .expect("storing a Container must succeed"),
        ),
        None => None,
    };
    plant_row(
        index,
        container_id,
        spool_path,
        SpoolState::Spooled,
        object_ref,
    )
    .await;
    container_id
}

/// Records one pending row by hand, in whichever state the case is about.
///
/// The spool file next to it is the caller's to write, to leave half-written, or
/// to leave out — which is the whole variation between the cases that plant a
/// row, so it is the one thing this does not decide.
async fn plant_row(
    index: &dyn Index,
    container_id: ContainerId,
    spool_path: PathBuf,
    state: SpoolState,
    object_ref: Option<ObjectRef>,
) {
    index
        .record_pending_upload(PendingUpload {
            container_id,
            spool_path,
            batch: BatchId::new("an-interrupted-run"),
            created_at: at(1),
            state,
            object_ref,
        })
        .await
        .expect("recording a pending upload must succeed");
}
