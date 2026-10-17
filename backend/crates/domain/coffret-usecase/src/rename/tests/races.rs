//! A rename racing another device's commit (spec: CP-4, CP-7, EP-7).
//!
//! The target device renames, and the source device's commit lands after the
//! rename planned and caught up, just before it consumes its slot. The rename
//! then rebases onto that commit by the rules EP-7 gives a pending rename.

use super::*;

const COVER: &[u8] = b"a cover";
const EDITED: &[u8] = b"a cover, edited on the other device";

/// The library the races run in: one file, synced by the source device.
async fn a_library(fixture: &FetchUnderTest, keys: &LibraryKeys) {
    write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/cover.jpg",
        COVER,
    );
    sync_source(fixture, keys, 1).await;
}

/// The target device's request, which relocates nothing: it maps no folder.
fn target_request<'a>(
    store: &'a dyn ObjectStore,
    fixture: &'a FetchUnderTest,
    keys: &'a LibraryKeys,
    to: &str,
) -> RenameRequest<'a> {
    RenameRequest::new(
        store,
        fixture.target(),
        keys,
        entry_path("albums/cover.jpg"),
        entry_path(to),
    )
    .with_policy(policy())
}

// EP-7 rule 3: an update of the renamed Entry landing first replaces its
// Container; the name the rename was prepared against now names the update's
// Entry, so the rename follows it and commits.
#[tokio::test]
async fn a_rename_follows_an_update_that_landed_first() {
    let fixture = fixture().await;
    let keys = keys();
    a_library(&fixture, &keys).await;

    let rival = Box::pin(async {
        let path = write(
            fixture.fs(),
            fixture.source_folder(),
            "albums/cover.jpg",
            EDITED,
        );
        touch(fixture.fs(), &path, NEWER);
        sync_source(&fixture, &keys, 2).await;
    });
    let store = RecordingStore::around(fixture.store()).racing(rival);
    let outcome = rename_entries(target_request(&store, &fixture, &keys, "albums/front.jpg"))
        .await
        .unwrap_or_else(|error| panic!("the rename must follow the update: {error}"));

    assert_eq!(outcome.commit.attempts, 2, "one rebase");
    let updated = entry_at(fixture.target(), "albums/front.jpg").await;
    assert_eq!(
        outcome.commit.record.renames()[0].container_id(),
        updated.container_id,
        "the rename names the update's Entry",
    );
    assert_eq!(
        updated.entry.extent.size(),
        EDITED.len() as u64,
        "the edited content"
    );
    assert_eq!(store.containers_put(), Vec::<String>::new());
    assert!(!holds(fixture.target(), "albums/cover.jpg").await);
}

// EP-7 rule 4: another device's rename of the same Entry landing first leaves
// two renames of one Entry, and the rename is refused as a conflict with
// nothing of its own committed.
#[tokio::test]
async fn a_rename_of_the_same_entry_landing_first_is_a_conflict() {
    let fixture = fixture().await;
    let keys = keys();
    a_library(&fixture, &keys).await;

    let rival = Box::pin(async {
        rename_entries(request(
            fixture.store(),
            fixture.source(),
            &fixture,
            &keys,
            "albums/cover.jpg",
            "albums/back.jpg",
        ))
        .await
        .unwrap_or_else(|error| panic!("the rival rename must commit: {error}"));
    });
    let store = RecordingStore::around(fixture.store()).racing(rival);
    let result = rename_entries(target_request(&store, &fixture, &keys, "albums/front.jpg")).await;

    match &result {
        Err(RenameError::Commit(failure)) => assert!(
            matches!(*failure.error, CommitError::RebaseConflict { .. }),
            "expected a rebase conflict, got {failure:?}",
        ),
        other => panic!("expected the rename to be refused as a conflict, got {other:?}"),
    }
    assert!(holds(fixture.target(), "albums/back.jpg").await);
    assert!(!holds(fixture.target(), "albums/front.jpg").await);
}
