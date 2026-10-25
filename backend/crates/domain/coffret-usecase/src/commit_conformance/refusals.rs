use coffret_model::{ContainerKind, ControlObjectName, Generation};

use crate::commit::{commit_batch, CommitError, PreparedBatch};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::faulty_store::FaultyStore;
use crate::commit_conformance::fixtures::{
    container_id, control_keys, path, prepared, request, request_under, triplicate,
};
use crate::commit_conformance::library::{mapped, Library};
use crate::commit_conformance::vanishing_head::VanishingHead;
use crate::error::Error;
use crate::generations::generation;

/// The generation a newly created Library's first commit takes, and its
/// Keyring generation with it: the one after the Library's first head
/// (spec: CP-2, FM-13).
fn generation_of_the_first_commit() -> Generation {
    generation(1)
}

/// The name a newly created Library's first commit is created under.
fn first_commit() -> ControlObjectName {
    ControlObjectName::head(generation_of_the_first_commit())
}

/// A batch whose Entry Paths would collide is refused, and nothing is written
/// (spec: EP-5, EP-6).
///
/// Refusing early is the point. The check could equally be made after the
/// Keyring candidate is on Storage and the answer would be the same, except
/// that the Library would then be carrying replicas no commit will ever select
/// — an orphan for OC-2 to reason about, made by a batch that never had a
/// chance. So the assertion is not only on the error but on the store holding
/// exactly what it held before.
pub async fn a_colliding_entry_path_is_refused_before_any_write(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();
    let before = Library::read(store).await.names();

    let batch = PreparedBatch::adding(vec![
        prepared(1, ContainerKind::OneFile, &["albums/a.jpg"]),
        prepared(2, ContainerKind::OneFile, &["albums/a.jpg"]),
    ]);
    let result = commit_batch(request(store, fixture.index(), &keys, batch)).await;

    match result.map_err(|failure| *failure.error) {
        Err(CommitError::EntryPathCollision { ref path }) => {
            assert_eq!(path.as_str(), "albums/a.jpg")
        }
        other => panic!("expected the colliding Entry Path to be refused, got {other:?}"),
    }
    assert_eq!(
        Library::read(store).await.names(),
        before,
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

    match result.map_err(|failure| *failure.error) {
        Err(CommitError::IncompleteKeyring {
            generation,
            replica,
            ..
        }) => {
            assert_eq!(generation, generation_of_the_first_commit());
            assert_eq!(replica, 1, "the failure names the replica that was missing");
        }
        other => panic!("expected an incomplete candidate Keyring, got {other:?}"),
    }
    assert!(
        !Library::read(store).await.holds(&first_commit()),
        "no Journal record is created while the candidate is incomplete",
    );
}

/// Where several replicas fail to read back, the commit names the lowest of
/// them, and writes no head (spec: CP-8, KL-2).
///
/// The replicas are written and read back together, so which failure Storage
/// answered first is a matter of timing. The error is not: it names the lowest
/// position that failed, so the same Library fails the same way however the
/// requests interleaved.
pub async fn the_lowest_missing_replica_is_the_one_reported(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let faulty = FaultyStore::swallowing_replicas(store, &[2, 1]);
    let keys = control_keys();

    let batch = PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
    let result = commit_batch(request_under(
        &faulty,
        fixture.index(),
        &keys,
        batch,
        triplicate(),
    ))
    .await;

    match result.map_err(|failure| *failure.error) {
        Err(CommitError::IncompleteKeyring {
            generation,
            replica,
            ..
        }) => {
            assert_eq!(generation, generation_of_the_first_commit());
            assert_eq!(replica, 1, "the failure names the lowest replica missing");
        }
        other => panic!("expected an incomplete candidate Keyring, got {other:?}"),
    }
    assert!(
        !Library::read(store).await.holds(&first_commit()),
        "no Journal record is created while the candidate is incomplete",
    );
}

/// A replica write Storage refuses stops the commit before its head, wherever
/// in the set the refused position is (spec: CP-1, CP-8).
///
/// The writes go out together, so the refused one may be answered before or
/// after the others land. Either way the commit has no complete candidate to
/// select, and the head chain stays where it was.
pub async fn a_refused_replica_write_stops_the_commit_before_its_head(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    for position in [0, 2] {
        let refusing = FaultyStore::refusing_replica_writes(store, position);
        let batch =
            PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
        let result = commit_batch(request_under(
            &refusing,
            fixture.index(),
            &keys,
            batch,
            triplicate(),
        ))
        .await
        .map_err(|failure| *failure.error);

        assert!(
            matches!(
                result,
                Err(CommitError::Storage(Error::PermissionDenied { .. }))
            ),
            "expected the refused write of replica {position} to stop the commit, got {result:?}",
        );
        assert!(
            !Library::read(store).await.holds(&first_commit()),
            "a commit whose replica {position} was refused creates no Journal record",
        );
    }
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
    let result = commit_batch(request(&refusing, index, &keys, batch.clone()))
        .await
        .map_err(|failure| *failure.error);
    assert!(
        matches!(result, Err(CommitError::Storage(Error::Rejected { .. }))),
        "expected the create of the record to be refused, got {result:?}",
    );

    let interrupted = Library::read(store).await;
    assert!(
        !interrupted.holds(&first_commit()),
        "a commit that never created its record left the head chain alone",
    );
    assert!(
        interrupted.holds_any("key-1-"),
        "the candidate it had already written is still there, selecting nothing",
    );
    assert_eq!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .map(|checkpoint| checkpoint.head_generation()),
        Some(Generation::FIRST),
        "the Index stands at the head the commit caught up to: the batch changed nothing \
         (spec: CP-1)",
    );

    let outcome = commit_batch(request(store, index, &keys, batch))
        .await
        .expect("the next run commits the same batch cleanly");
    assert_eq!(outcome.record.generation(), generation(1));
    assert_eq!(outcome.record.prev(), Some(Generation::FIRST));

    let library = Library::read(store).await;
    let keyring = library.keyring(store, outcome.record.keyring()).await;
    assert_eq!(mapped(&keyring), vec![container_id(1)]);
}

/// CP-16: immediately before consuming a slot, a writer re-reads the head
/// object the slot came from and aborts if it is gone.
///
/// The head disappears after the catch-up has read it and before its slot is
/// consumed, which is the window a writer that slept through a later epoch's
/// rotation would wake into. The commit stops with the missing head rather
/// than creating a successor into a position the Library no longer holds, and
/// no successor exists afterwards.
pub async fn a_head_gone_before_its_slot_is_consumed_stops_the_commit(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let first = PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
    commit_batch(request(store, index, &keys, first))
        .await
        .expect("the first commit must succeed");

    let head = Library::read(store)
        .await
        .handle(&ControlObjectName::head(generation(1)));
    let vanishing = VanishingHead::purging(store, head);
    Library::upload_container(store, container_id(2)).await;
    let second =
        PreparedBatch::adding(vec![prepared(2, ContainerKind::OneFile, &["albums/b.jpg"])]);
    let result = commit_batch(request(&vanishing, index, &keys, second))
        .await
        .map_err(|failure| *failure.error);

    assert!(
        vanishing.has_purged(),
        "the head was taken away while the commit ran"
    );
    match result {
        Err(CommitError::MissingHead {
            generation: missing,
        }) => {
            assert_eq!(
                missing,
                generation(1),
                "the refusal names the head that went"
            )
        }
        other => panic!("expected the missing head to stop the commit, got {other:?}"),
    }
    assert!(
        !Library::read(store)
            .await
            .holds(&ControlObjectName::head(generation(2))),
        "no successor was created into the slot of a head that is gone",
    );
}

/// OC-6: a removal Storage will not trash still commits, and the outcome says
/// why it was not trashed (spec: CP-14).
///
/// Trashing happens after the commit point, so a provider that refuses it
/// un-commits nothing: the record stands, the Container is out of the current
/// set, and what is left behind is an object no current state names. The device
/// that could not move it is what has to finish the job later, and "which
/// Container" is not enough to decide how: credentials that may write but not
/// delete need a person, where a provider having a bad minute needs only another
/// run. So the refusal comes back in the outcome beside the Container ID, rather
/// than living out its life in a diagnostic event.
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

    assert_eq!(outcome.record.generation(), generation(2));
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
