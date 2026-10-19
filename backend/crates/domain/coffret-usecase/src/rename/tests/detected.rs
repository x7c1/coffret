//! A rename a sync detected on disk, as another device meets it: committed
//! against a head that moved meanwhile (spec: CP-4, EP-7), and followed by a
//! device that fetched the file earlier (spec: EP-10).
//!
//! The suite over the sync holds the detection itself to both Index
//! implementations; these cases need a second device, which that suite does
//! not have, and so live beside the rename's own.

use crate::fetch_conformance::fixtures::at;
use crate::sync::{sync_folders, SyncError, SyncOutcome, SyncRequest, SyncResult};

use super::*;

const COVER: &[u8] = b"a cover kept in a Container of its own";
const NOTES: &[u8] = b"notes that another device renames meanwhile";

/// One sync of the source device's folder through `store`.
async fn sync_through(
    store: &dyn ObjectStore,
    fixture: &FetchUnderTest,
    keys: &LibraryKeys,
    run: i64,
) -> SyncResult<SyncOutcome> {
    sync_folders(
        SyncRequest::new(
            store,
            fixture.source(),
            keys,
            fixture.fs(),
            fixture.fs(),
            fixture.fs(),
            fixture.fs(),
            fixture.spool_dir(),
            crate::device_state::BatchId::new(format!("run-{run}")),
            at(run),
        )
        .with_policy(policy()),
    )
    .await
}

/// Renames a file in the source device's folder the way a person does: the
/// same bytes and modification time, at another name.
fn rename_on_disk(fixture: &FetchUnderTest, from: &str, to: &str) {
    let from = fixture.source_folder().join(from);
    let content = read(fixture.fs(), &from);
    let (_, mtime) = fixture
        .fs()
        .observed(&from)
        .expect("a case renames a file it wrote");
    fixture.fs().remove_file(&from);
    let to = fixture.source_folder().join(to);
    fixture.fs().write_file(&to, &content);
    fixture.fs().set_mtime(&to, mtime.as_unix_seconds());
}

/// The library the cases run in: two files, synced by the source device.
async fn a_library(fixture: &FetchUnderTest, keys: &LibraryKeys) {
    write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/cover.jpg",
        COVER,
    );
    write(fixture.fs(), fixture.source_folder(), "notes.txt", NOTES);
    sync_source(fixture, keys, 1).await;
}

// CP-4, EP-7: another device's commit that touches nothing the detected rename
// names lands first; the sync's batch is rebased onto it and its rename
// commits, uploading nothing.
#[tokio::test]
async fn a_detected_rename_is_rebased_onto_an_unrelated_commit() {
    let fixture = fixture().await;
    let keys = keys();
    a_library(&fixture, &keys).await;
    rename_on_disk(&fixture, "albums/cover.jpg", "albums/front.jpg");

    let rival = Box::pin(async {
        rename_entries(
            RenameRequest::new(
                fixture.store(),
                fixture.target(),
                &keys,
                fixture.fs(),
                entry_path("notes.txt"),
                entry_path("journal.txt"),
                renamed_at(),
            )
            .with_policy(policy()),
        )
        .await
        .unwrap_or_else(|error| panic!("the rival rename must commit: {error}"));
    });
    let store = RecordingStore::around(fixture.store()).racing(rival);
    let outcome = sync_through(&store, &fixture, &keys, 2)
        .await
        .unwrap_or_else(|error| panic!("the sync must rebase and commit: {error}"));

    let commit = outcome.commit.expect("the rename is committed");
    assert_eq!(commit.attempts, 2, "one rebase");
    assert_eq!(
        outcome.renamed,
        [renamed("albums/cover.jpg", "albums/front.jpg")]
    );
    assert_eq!(store.containers_put(), Vec::<String>::new());
    assert!(holds(fixture.source(), "albums/front.jpg").await);
    assert!(holds(fixture.source(), "journal.txt").await);
    assert!(!holds(fixture.source(), "albums/cover.jpg").await);
}

// EP-7 rule 4: another device's rename of the same Entry landing first leaves
// two renames of one Entry, and the sync's batch is refused as a conflict with
// nothing of its own committed.
#[tokio::test]
async fn a_detected_rename_racing_a_rename_of_the_same_entry_is_a_conflict() {
    let fixture = fixture().await;
    let keys = keys();
    a_library(&fixture, &keys).await;
    rename_on_disk(&fixture, "albums/cover.jpg", "albums/front.jpg");

    let rival = Box::pin(async {
        rename_entries(
            RenameRequest::new(
                fixture.store(),
                fixture.target(),
                &keys,
                fixture.fs(),
                entry_path("albums/cover.jpg"),
                entry_path("albums/back.jpg"),
                renamed_at(),
            )
            .with_policy(policy()),
        )
        .await
        .unwrap_or_else(|error| panic!("the rival rename must commit: {error}"));
    });
    let store = RecordingStore::around(fixture.store()).racing(rival);
    let result = sync_through(&store, &fixture, &keys, 2).await;

    match &result {
        Err(SyncError::Commit(failure)) => assert!(
            matches!(*failure.error, CommitError::RebaseConflict { .. }),
            "expected a rebase conflict, got {failure:?}",
        ),
        other => panic!("expected the sync to be refused as a conflict, got {other:?}"),
    }
    assert_eq!(store.containers_put(), Vec::<String>::new());
    assert!(holds(fixture.target(), "albums/back.jpg").await);
    assert!(!holds(fixture.target(), "albums/front.jpg").await);
}

// EP-10: a device that fetched the file before it was renamed on the source
// device relocates its own copy to follow the rename, reading no Container.
#[tokio::test]
async fn another_device_follows_a_detected_rename_without_reading_a_container() {
    let fixture = fixture().await;
    let keys = keys();
    a_library(&fixture, &keys).await;
    map(
        fixture.target(),
        fixture.fs(),
        None,
        fixture.target_folder(),
    )
    .await;
    fetch_folders(fetch_request(fixture.store(), &fixture, &keys, 2))
        .await
        .unwrap_or_else(|error| panic!("the first fetch must succeed: {error}"));

    rename_on_disk(&fixture, "albums/cover.jpg", "albums/front.jpg");
    let synced = sync_through(fixture.store(), &fixture, &keys, 3)
        .await
        .unwrap_or_else(|error| panic!("the sync must commit the rename: {error}"));
    assert_eq!(
        synced.renamed,
        [renamed("albums/cover.jpg", "albums/front.jpg")]
    );

    let store = RecordingStore::around(fixture.store());
    let outcome = fetch_folders(fetch_request(&store, &fixture, &keys, 4))
        .await
        .unwrap_or_else(|error| panic!("the fetch after the rename must succeed: {error}"));

    assert_eq!(store.containers_read(), Vec::<String>::new());
    assert!(outcome.fetched.is_empty(), "{:?}", outcome.fetched);
    assert_eq!(
        relocated(&outcome.relocations),
        [(
            entry_path("albums/cover.jpg"),
            entry_path("albums/front.jpg")
        )]
    );
    assert_eq!(
        file(&fixture, fixture.target_folder(), "albums/front.jpg").as_deref(),
        Some(COVER)
    );
    assert_eq!(
        file(&fixture, fixture.target_folder(), "albums/cover.jpg"),
        None
    );
}
