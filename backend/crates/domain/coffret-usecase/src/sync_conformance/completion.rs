use std::path::PathBuf;

use coffret_model::{ContainerId, EntryPath};

use crate::commit::CommitError;
use crate::conformance_library::Library;
use crate::device_state::LocalEntryState;
use crate::sync::{sync_folders, LibraryKeys, Reconciled, SyncError};
use crate::sync_conformance::counting_store::CountingStore;
use crate::sync_conformance::fixtures::{
    keys, map, master_key, observed, pending, request, spooled, touch, write, NEWER, OLDER,
};
use crate::sync_conformance::refusing_index::RefusingIndex;
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// The bytes the interrupted run committed, before anything is modified.
const ORIGINAL: &[u8] = b"the bytes the interrupted run committed";

/// A commit that landed and whose Index refresh did not converges in one run.
///
/// This is the state the whole rule exists for. The record is on Storage, so the
/// Container is current whether or not this device knows it (spec: CP-1), and
/// the refresh that would have said so — and would have marked the file present
/// and dropped the pending row — did not happen. The Library-wide half of it any
/// catch-up replays; the device-local half exists in one place only, which is
/// the row the interrupted run left behind (spec: EP-10, OC-2).
///
/// So the next run reads it there. It catches its Index up before it scans,
/// finds the row's Container current, and completes the interrupted bookkeeping
/// instead of reclaiming anything (spec: OC-7) — after which a file modified in
/// the meantime is an ordinary replacement, in one pass, with nothing uploaded
/// twice. Without the completion the same file would be spooled again as if it
/// had never been committed, and the commit's own catch-up would then refuse the
/// batch as a collision with this device's own Entry (spec: EP-6).
pub async fn a_commit_whose_refresh_failed_is_completed_and_replaced(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    let (landed, path) = interrupted_refresh(fixture, &keys).await;

    let changed = b"bytes that are not the ones the interrupted run committed".as_slice();
    tokio::fs::write(&path, changed)
        .await
        .expect("rewriting the file must succeed");
    touch(&path, NEWER);

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 2))
        .await
        .expect("a sync after an interrupted refresh must succeed");

    assert_eq!(
        outcome.reconciled,
        vec![Reconciled::Completed {
            container_id: landed,
            entries: 1,
        }],
        "the row's Container is current, so its commit landed (spec: OC-7)",
    );
    assert_eq!(
        outcome.added.len(),
        1,
        "the modified file is committed once, not once more than once",
    );
    assert_eq!(
        outcome.replaced,
        vec![landed],
        "the completed Container is what the modification replaces (spec: CP-14)",
    );

    let commit = outcome
        .commit
        .expect("a file modified since the interrupted run is worth a commit");
    assert_eq!(commit.record.additions.len(), 1, "one Entry, not two");
    assert_eq!(commit.record.removals, vec![landed]);

    assert!(
        pending(index).await.is_empty(),
        "nothing is left pending for a later run to find a third time",
    );
    assert_eq!(spooled(fixture.spool()).await, 0);

    let library = Library::read(store).await;
    assert!(
        !library.holds_container(landed),
        "the replaced Container's object leaves the listing",
    );
    let container = library
        .open(store, &commit.record, outcome.added[0], &master_key())
        .await;
    assert_eq!(
        container.entries[0].content, changed,
        "what another device can open is the file as it is on disk now",
    );
}

/// Completion alone puts the device back in step: the file is present.
///
/// The same interrupted state with nothing modified. Completion is not a
/// bookkeeping detail here — it is what makes the difference between a file this
/// device knows it materialized and a path it treats as never having been in its
/// scope, which is what silently excludes every later modification and deletion
/// of that file from every scan (spec: EP-10).
///
/// So what the run has to end at is a present row whose observation is the file
/// on disk, and a Library it changed nothing about: an unchanged file is worth no
/// generation (spec: CP-1).
pub async fn a_completed_container_marks_its_file_present(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    let (landed, path) = interrupted_refresh(fixture, &keys).await;

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 2))
        .await
        .expect("a sync after an interrupted refresh must succeed");

    assert_eq!(
        outcome.reconciled,
        vec![Reconciled::Completed {
            container_id: landed,
            entries: 1,
        }],
    );
    assert!(
        outcome.commit.is_none(),
        "the file is what the Library already holds, so no generation is spent",
    );
    assert!(outcome.added.is_empty());
    assert_eq!(
        outcome.unchanged, 1,
        "the observation the completion recorded is what the scan compares against",
    );

    let local = index
        .local_entry_at(&EntryPath::nfc("a.jpg"))
        .await
        .expect("asking the Index about a local file must succeed")
        .expect("completion records what the interrupted run put on disk (spec: EP-10)");
    assert_eq!(local.state, LocalEntryState::Present);
    let (size, mtime) = observed(&path).await;
    assert_eq!(local.observation.size, size);
    assert_eq!(local.observation.mtime, mtime);

    assert!(pending(index).await.is_empty());
    assert_eq!(spooled(fixture.spool()).await, 0);
    assert!(
        Library::read(store).await.holds_container(landed),
        "a completed Container's object is the Library's and is left where it is",
    );
}

/// A run with no pending rows asks Storage for nothing at all.
///
/// Settling before the scan is what makes the interrupted cases converge, and it
/// may not be paid for by the runs that have nothing to settle — which is every
/// ordinary run. The verdict a settling run needs is the Library's head, and the
/// row that needs it is the one naming an uploaded object; with no rows at all
/// there is no question to answer, so the Index is asked once and Storage is not
/// asked anything (spec: OC-6).
pub async fn a_run_with_no_pending_rows_reads_no_head(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let path = write(fixture.folder(), "a.jpg", ORIGINAL).await;
    touch(&path, OLDER);
    sync_folders(request(store, index, &keys, fixture.spool(), 1))
        .await
        .expect("a first sync must succeed");
    assert!(
        pending(index).await.is_empty(),
        "a committed batch leaves no rows behind (spec: OC-2)",
    );

    let counting = CountingStore::around(store);
    let outcome = sync_folders(request(&counting, index, &keys, fixture.spool(), 2))
        .await
        .expect("a second sync of an untouched folder must succeed");

    assert!(outcome.reconciled.is_empty());
    assert!(outcome.commit.is_none());
    assert_eq!(outcome.unchanged, 1);
    assert_eq!(
        counting.listings(),
        0,
        "no row to settle is no reason to walk Storage",
    );
}

/// Leaves the state a run whose record landed and whose refresh failed leaves.
///
/// Driven rather than planted, because what is being set up is an ordering
/// inside one call: the record has to exist and the refresh has to have failed
/// after it. The catalog the run writes into is the fixture's own, wrapped only
/// to refuse that one write, so what the case goes on to inspect really is what
/// the interrupted run left behind.
///
/// Answers with the Container whose commit landed, and the local file it holds.
async fn interrupted_refresh(
    fixture: &SyncUnderTest,
    keys: &LibraryKeys,
) -> (ContainerId, PathBuf) {
    let store = fixture.store();
    let index = fixture.index();
    map(fixture, None).await;

    let path = write(fixture.folder(), "a.jpg", ORIGINAL).await;
    touch(&path, OLDER);

    let refusing = RefusingIndex::around(index);
    let result = sync_folders(request(store, &refusing, keys, fixture.spool(), 1)).await;
    let Err(SyncError::Commit(CommitError::Index(_))) = &result else {
        panic!("a refused refresh must fail the run that committed, got {result:?}");
    };

    let rows = pending(index).await;
    assert_eq!(
        rows.len(),
        1,
        "the row of the committed Container survives the failed refresh (spec: OC-2)",
    );
    assert!(
        rows[0].object_ref.is_some(),
        "the run got as far as uploading, and past it",
    );
    assert_eq!(
        spooled(fixture.spool()).await,
        1,
        "the spool the refresh would have cleared is still there",
    );
    assert!(
        index
            .entry_at(&EntryPath::nfc("a.jpg"))
            .await
            .expect("asking the Index for a path must succeed")
            .is_none(),
        "this device's catalog is behind the record it committed",
    );
    assert!(
        index
            .local_entry_at(&EntryPath::nfc("a.jpg"))
            .await
            .expect("asking the Index about a local file must succeed")
            .is_none(),
        "and behind the file it put on disk (spec: EP-10)",
    );
    (rows[0].container_id, path)
}
