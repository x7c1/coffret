use coffret_model::{ContainerKind, ControlObjectName};

use crate::commit::{commit_batch, CommitError, PreparedBatch};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::fixtures::{container_id, control_keys, path, prepared, request};
use crate::commit_conformance::library::Library;
use crate::commit_conformance::racing_store::RacingStore;
use crate::generations::generation;

/// A rebased batch whose removal another writer already made is refused, and
/// the head the other writer left is the one that stands (spec: CP-7, CP-18).
///
/// The writer under test replaces Container 1; the rival removes it outright,
/// adding nothing, and lands first, at the moment the writer reaches the create
/// of its record. Rebasing the replacement onto that head and committing it
/// would bring the deleted Entry back as if the deletion had never happened, and
/// nothing in the Entry Path recheck stops it: the path is free once Container 1
/// is no longer current (spec: EP-6, EP-7). So the rebase refuses the batch, naming the Container,
/// and writes no record of its own.
pub async fn a_rebased_batch_whose_removal_is_no_longer_current_is_refused(
    fixture: &CommitUnderTest,
) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();

    for seed in [1, 3] {
        Library::upload_container(store, container_id(seed)).await;
    }
    let first = PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
    commit_batch(request(store, index, &keys, first))
        .await
        .expect("the first commit must succeed");

    let deletion = PreparedBatch::default().removing(vec![container_id(1)]);
    let racing = RacingStore::letting_in(store, fixture.other(), &keys, deletion);

    let replacement =
        PreparedBatch::adding(vec![prepared(3, ContainerKind::OneFile, &["albums/a.jpg"])])
            .removing(vec![container_id(1)]);
    let result = commit_batch(request(&racing, index, &keys, replacement))
        .await
        .map_err(|failure| *failure.error);

    match result {
        Err(CommitError::RemovalNotCurrent { ref container_ids }) => {
            assert_eq!(container_ids, &vec![container_id(1)]);
        }
        other => panic!("expected the stale removal to be refused, got {other:?}"),
    }

    let library = Library::read(store).await;
    let rival = library.record(store, generation(1)).await;
    assert_eq!(
        rival.removals(),
        vec![container_id(1)],
        "the head is the rival's deletion",
    );
    assert!(rival.additions().is_empty());
    assert!(
        !library.holds(&ControlObjectName::head(generation(2))),
        "the refused batch wrote no record of its own (spec: CP-1)",
    );
    assert!(
        index
            .entry_at(&path("albums/a.jpg"))
            .await
            .expect("asking the Index for a path must succeed")
            .is_none(),
        "the deleted Entry stays deleted",
    );
    assert!(
        library.holds_container(container_id(3)),
        "the refused replacement's upload is left for its producer to settle (spec: OC-2)",
    );
}

/// A rebased batch whose removals are all still current commits as it always
/// has (spec: CP-4, CP-18).
///
/// The rival changes something else entirely, so the rebase finds Container 1
/// exactly as current as the batch was prepared to find it, and the
/// replacement lands on the head after the rival's.
pub async fn a_rebased_batch_whose_removals_are_current_commits(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();

    for seed in [1, 3, 4] {
        Library::upload_container(store, container_id(seed)).await;
    }
    let first = PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
    commit_batch(request(store, index, &keys, first))
        .await
        .expect("the first commit must succeed");

    let elsewhere =
        PreparedBatch::adding(vec![prepared(4, ContainerKind::OneFile, &["books/b.png"])]);
    let racing = RacingStore::letting_in(store, fixture.other(), &keys, elsewhere);

    let replacement =
        PreparedBatch::adding(vec![prepared(3, ContainerKind::OneFile, &["albums/a.jpg"])])
            .removing(vec![container_id(1)]);
    let outcome = commit_batch(request(&racing, index, &keys, replacement))
        .await
        .expect("a removal that is still current rebases and commits");

    assert_eq!(outcome.attempts, 2, "the first attempt lost the slot");
    assert_eq!(outcome.record.generation(), generation(2));
    assert_eq!(outcome.record.prev(), Some(generation(1)));
    assert_eq!(outcome.record.removals(), vec![container_id(1)]);

    let moved = index
        .entry_at(&path("albums/a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the path is current, held by the replacement");
    assert_eq!(moved.container_id, container_id(3));
    assert!(!Library::read(store).await.holds_container(container_id(1)));
}
