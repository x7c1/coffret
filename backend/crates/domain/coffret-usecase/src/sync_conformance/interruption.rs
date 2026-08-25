use std::path::{Path, PathBuf};

use coffret_format::generate_container_id;
use coffret_model::{ContainerId, EntryPath};

use crate::byte_stream::ByteStream;
use crate::conformance_library::Library;
use crate::device_state::{BatchId, PendingUpload};
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync::{sync_folders, Reconciled};
use crate::sync_conformance::fixtures::{at, keys, map, pending, request, spooled, write};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

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
        .entry_at(&EntryPath::new("a.jpg"))
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
/// And it reads it before the scan, because a row left open is exactly what
/// makes a scan spool a file this device may already have committed.
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
/// Nothing about it is recoverable and nothing about it is on Storage, so the
/// row is bookkeeping for a Container that no longer exists anywhere. Dropping
/// one that is already half gone is idempotent, which is what lets an
/// interrupted cleanup simply be run again (spec: OC-6).
pub async fn a_stale_pending_row_is_dropped_with_its_spool(fixture: &SyncUnderTest) {
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let container_id = generate_container_id().expect("the OS CSPRNG is available");
    index
        .record_pending_upload(PendingUpload {
            container_id,
            spool_path: fixture.spool().join("a-spool-that-is-not-there"),
            batch: BatchId::new("an-interrupted-run"),
            created_at: at(0),
            object_ref: None,
        })
        .await
        .expect("recording a pending upload must succeed");

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
    index
        .record_pending_upload(PendingUpload {
            container_id,
            spool_path,
            batch: BatchId::new("an-interrupted-run"),
            created_at: at(1),
            object_ref,
        })
        .await
        .expect("recording a pending upload must succeed");
    container_id
}
