use coffret_model::{ContainerKind, Generation};

use crate::commit::{commit_batch, CommitError, PreparedBatch};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::faulty_store::FaultyStore;
use crate::commit_conformance::fixtures::{container_id, control_keys, path, prepared, request};
use crate::commit_conformance::library::{mapped, Library};
use crate::error::Error;
use crate::generations::generation;

/// A batch whose Entry Paths would collide is refused, and nothing is written
/// (spec: EP-5, EP-6).
///
/// Refusing early is the point. The check could equally be made after the
/// Keyring candidate is on Storage and the answer would be the same, except
/// that the Library would then be carrying replicas no commit will ever select
/// — an orphan for OC-2 to reason about, made by a batch that never had a
/// chance. So the assertion is not only on the error but on the store being
/// exactly as empty as it started.
pub async fn a_colliding_entry_path_is_refused_before_any_write(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    let batch = PreparedBatch::adding(vec![
        prepared(1, ContainerKind::OneFile, &["albums/a.jpg"]),
        prepared(2, ContainerKind::OneFile, &["albums/a.jpg"]),
    ]);
    let result = commit_batch(request(store, fixture.index(), &keys, batch)).await;

    match result {
        Err(CommitError::EntryPathCollision { ref path }) => {
            assert_eq!(path.as_str(), "albums/a.jpg")
        }
        other => panic!("expected the colliding Entry Path to be refused, got {other:?}"),
    }
    assert!(
        Library::read(store).await.names().is_empty(),
        "a refused batch writes nothing at all",
    );
}

/// A Keyring replica that does not read back stops the commit (spec: CP-8,
/// KL-2).
///
/// The candidate set has to be complete before a commit may select it, and
/// complete means every declared replica index is present and valid — not that
/// every write was acknowledged. A provider that answers a write and loses the
/// object is exactly why the read-back exists, so that is what is simulated
/// here, and the Journal must be untouched afterwards.
pub async fn a_missing_keyring_replica_stops_the_commit(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let faulty = FaultyStore::swallowing_replica(store, 1);
    let keys = control_keys();

    let batch = PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
    let result = commit_batch(request(&faulty, fixture.index(), &keys, batch)).await;

    match result {
        Err(CommitError::IncompleteKeyring {
            generation,
            replica,
            ..
        }) => {
            assert_eq!(generation, Generation::FIRST);
            assert_eq!(replica, 1, "the failure names the replica that was missing");
        }
        other => panic!("expected an incomplete candidate Keyring, got {other:?}"),
    }
    assert!(
        !Library::read(store).await.holds_any("head-"),
        "no Journal record is created while the candidate is incomplete",
    );
}

/// An interrupted commit leaves the head where it was, and the next run commits
/// (spec: CP-1, KL-3).
///
/// The Keyring candidate is already on Storage when the record fails to be
/// created, and it stays there — an uncommitted set selects nothing, so it is
/// harmless rather than something to unwind. What matters is that the head
/// chain did not move: nothing about the Library changed, and a device that
/// simply runs again lands on the generation the interrupted attempt was aiming
/// at.
pub async fn an_interrupted_commit_leaves_the_head_unchanged(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let batch = PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);

    let refusing = FaultyStore::refusing_the_head(store);
    let result = commit_batch(request(&refusing, index, &keys, batch.clone())).await;
    assert!(
        matches!(result, Err(CommitError::Storage(Error::Rejected { .. }))),
        "expected the create of the record to be refused, got {result:?}",
    );

    let interrupted = Library::read(store).await;
    assert!(
        !interrupted.holds_any("head-"),
        "a commit that never created its record left the head chain alone",
    );
    assert!(
        interrupted.holds_any("key-"),
        "the candidate it had already written is still there, selecting nothing",
    );
    assert!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .is_none(),
        "the Index stands where it did: the batch changed nothing (spec: CP-1)",
    );

    let outcome = commit_batch(request(store, index, &keys, batch))
        .await
        .expect("the next run commits the same batch cleanly");
    assert_eq!(outcome.record.generation(), Generation::FIRST);
    assert_eq!(outcome.record.prev(), None);

    let library = Library::read(store).await;
    let keyring = library.keyring(store, outcome.record.keyring()).await;
    assert_eq!(mapped(&keyring), vec![container_id(1)]);
}

/// A removal Storage will not trash still commits, and the outcome says why it
/// was not trashed (spec: CP-14, OC-6).
///
/// Trashing happens after the commit point, so a provider that refuses it
/// un-commits nothing: the record stands, the Container is out of the current
/// set, and what is left behind is an object no current state names. The device
/// that could not move it is what has to finish the job later, and "which
/// Container" is not enough to decide how: credentials that may write but not
/// delete need a person, where a provider having a bad minute needs only another
/// run. So the refusal comes back in the outcome beside the Container ID, rather
/// than living out its life in a log line.
pub async fn an_untrashed_removal_reports_what_storage_refused(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();

    for seed in [1, 2] {
        Library::upload_container(store, container_id(seed)).await;
    }
    let first = PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
    commit_batch(request(store, index, &keys, first))
        .await
        .expect("the first commit must succeed");

    let refusing = FaultyStore::refusing_to_trash(store);
    let second =
        PreparedBatch::adding(vec![prepared(2, ContainerKind::OneFile, &["albums/a.jpg"])])
            .removing(vec![container_id(1)]);
    let outcome = commit_batch(request(&refusing, index, &keys, second))
        .await
        .expect("a trash the provider refuses does not fail the commit");

    assert_eq!(outcome.record.generation(), generation(1));
    assert_eq!(outcome.record.removals(), vec![container_id(1)]);
    let [untrashed] = &outcome.untrashed[..] else {
        panic!(
            "one removal was refused, so one is reported, got {:?}",
            outcome.untrashed,
        );
    };
    assert_eq!(
        untrashed.container_id,
        container_id(1),
        "the outcome names the Container still in Storage",
    );
    assert!(
        matches!(untrashed.cause, Error::PermissionDenied { .. }),
        "the outcome carries the refusal Storage answered with, got {:?}",
        untrashed.cause,
    );

    assert!(
        Library::read(store).await.holds_container(container_id(1)),
        "the object the provider would not move is exactly where it was",
    );
    let moved = index
        .entry_at(&path("albums/a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the path is current, held by the replacement");
    assert_eq!(
        moved.container_id,
        container_id(2),
        "the record is the truth about what is current, whatever Storage still holds",
    );
}
