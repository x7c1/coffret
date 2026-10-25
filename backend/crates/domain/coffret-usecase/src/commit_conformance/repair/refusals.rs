use coffret_model::ControlObjectName;

use crate::commit::{commit_batch, CommitError, UnrepairedReplica, UnusableReplica};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::counting_store::CountingStore;
use crate::commit_conformance::faulty_store::FaultyStore;
use crate::commit_conformance::fixtures::{
    container_id, control_keys, request, request_under, triplicate,
};
use crate::commit_conformance::library::{lose_replica, mangle_replica, replica_name, Library};
use crate::error::Error;
use crate::generations::generation;

use super::{adding, one_repair, replicas_written};

/// A repair Storage will not take the write of refuses the commit, and commits
/// nothing (spec: KL-11, KL-16).
///
/// The gate is never partially relaxed: a set one replica short is one no write
/// may go past, so the batch stops before its own Keyring candidate is written
/// and long before the Journal is touched. Nothing is left behind for a later
/// run to reason about, the Index stands where it did, and running again with
/// the provider back is the whole of the recovery — which the case checks by
/// doing exactly that.
pub async fn a_repair_the_provider_refuses_stops_the_commit(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let first = commit_batch(request(store, index, &keys, adding(1)))
        .await
        .expect("a commit into a newly created Library must succeed");
    let committed = first.record.keyring().clone();

    lose_replica(store, &committed, 1).await;
    Library::upload_container(store, container_id(2)).await;

    let refusing = FaultyStore::refusing_replica_writes(store, 1);
    let counting = CountingStore::around(&refusing);
    let result = commit_batch(request(&counting, index, &keys, adding(2))).await;

    match result.map_err(|failure| (*failure.error, failure.repairs)) {
        Err((
            CommitError::UnrepairedKeyring {
                generation,
                ref needed,
                ref rewritten,
                replica,
                cause: UnrepairedReplica::Unwritten(ref error),
            },
            ref repairs,
        )) => {
            assert!(
                repairs.is_empty(),
                "what the refusing examination put back is on `rewritten`, about the \
                 generation the refusal names, and no earlier attempt repaired \
                 anything, got {repairs:?}",
            );
            assert_eq!(generation, committed.generation());
            assert_eq!(needed, &vec![1], "the position still short is named");
            assert!(
                rewritten.is_empty(),
                "the one short position is the one that was refused, so nothing \
                 was put back, got {rewritten:?}",
            );
            assert_eq!(replica, 1);
            assert!(
                matches!(
                    error.as_ref(),
                    CommitError::Storage(Error::PermissionDenied { .. })
                ),
                "what Storage refused the write with travels inside, got {error:?}",
            );
        }
        other => panic!("expected the degraded set to refuse the commit, got {other:?}"),
    }

    let refused = Library::read(store).await;
    assert!(
        !refused.holds(&ControlObjectName::head(generation(2))),
        "nothing of the batch was committed (spec: KL-16)",
    );
    assert!(
        replicas_written(&counting, generation(2)).is_empty(),
        "and the batch's own Keyring candidate was never written either",
    );
    assert_eq!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .expect("the first commit left one")
            .head_generation(),
        generation(1),
        "the Index stands where the first commit left it (spec: CP-1)",
    );

    // The next run, with the provider taking writes again.
    let outcome = commit_batch(request(store, index, &keys, adding(2)))
        .await
        .expect("a later run repairs the set and commits the same batch");
    assert_eq!(
        one_repair(&outcome).rewritten.iter().collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(outcome.record.generation(), generation(2));
    Library::read(store).await.keyring(store, &committed).await;
}

/// A repair that stops still reports the positions it put back (spec: KL-15,
/// KL-16).
///
/// Every position in need is attempted rather than the walk stopping at the
/// first that fails, so a set two replicas short can leave the run one short:
/// the commit is refused all the same, and the rewrite that succeeded stands.
/// That is a repair performed, which is never silent — and the refusal is the
/// only thing a refused run hands its caller, there being no outcome to put it
/// on, so it is carried there or it is carried nowhere.
pub async fn a_repair_that_stops_reports_what_it_put_back(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let first = commit_batch(request_under(store, index, &keys, adding(1), triplicate()))
        .await
        .expect("a commit into a newly created Library must succeed");
    let committed = first.record.keyring().clone();

    for position in [1, 2] {
        lose_replica(store, &committed, position).await;
    }
    Library::upload_container(store, container_id(2)).await;

    let refusing = FaultyStore::refusing_replica_writes(store, 2);
    let result = commit_batch(request_under(
        &refusing,
        index,
        &keys,
        adding(2),
        triplicate(),
    ))
    .await;

    match result.map_err(|failure| (*failure.error, failure.repairs)) {
        Err((
            CommitError::UnrepairedKeyring {
                generation,
                ref needed,
                ref rewritten,
                replica,
                cause: UnrepairedReplica::Unwritten(_),
            },
            ref repairs,
        )) => {
            assert!(
                repairs.is_empty(),
                "what the refusing examination put back is on `rewritten`, about the \
                 generation the refusal names, and no earlier attempt repaired \
                 anything, got {repairs:?}",
            );
            assert_eq!(generation, committed.generation());
            assert_eq!(needed, &vec![2], "the position the provider refused");
            assert_eq!(
                rewritten,
                &vec![1],
                "and the one the same examination put back before it stopped",
            );
            assert_eq!(replica, 2);
        }
        other => panic!("expected the refused write to stop the commit, got {other:?}"),
    }

    let library = Library::read(store).await;
    assert!(
        library.holds(&replica_name(&committed, 1)),
        "the rewrite that was taken stands, so the next run meets a set one \
         replica short rather than two",
    );
    assert!(
        !library.holds(&ControlObjectName::head(generation(2))),
        "and nothing of the batch was committed (spec: KL-16)",
    );
}

/// A replica Storage will not hand over refuses the commit and is not rewritten
/// (spec: KL-13, KL-16).
///
/// The one short position a repair may not act on. A fetch that failed says
/// nothing about the object: it may be exactly the replica its name promises,
/// and writing over it on that evidence would be a device deciding a replica is
/// lost because it could not reach it. So the position is left exactly as it
/// stands — and the set still cannot be called complete, so the gate holds shut
/// all the same.
pub async fn an_unfetchable_replica_stops_the_commit_unrewritten(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let first = commit_batch(request(store, index, &keys, adding(1)))
        .await
        .expect("a commit into a newly created Library must succeed");
    let committed = first.record.keyring().clone();

    let hidden = Library::read(store)
        .await
        .handle(&replica_name(&committed, 1));
    Library::upload_container(store, container_id(2)).await;

    let hiding = FaultyStore::hiding(store, &hidden);
    let counting = CountingStore::around(&hiding);
    let result = commit_batch(request(&counting, index, &keys, adding(2))).await;

    match result.map_err(|failure| (*failure.error, failure.repairs)) {
        Err((
            CommitError::UnrepairedKeyring {
                generation,
                ref needed,
                ref rewritten,
                replica,
                cause: UnrepairedReplica::Unfetchable(_),
            },
            ref repairs,
        )) => {
            assert!(
                repairs.is_empty(),
                "what the refusing examination put back is on `rewritten`, about the \
                 generation the refusal names, and no earlier attempt repaired \
                 anything, got {repairs:?}",
            );
            assert_eq!(generation, committed.generation());
            assert_eq!(needed, &vec![1]);
            assert!(rewritten.is_empty(), "got {rewritten:?}");
            assert_eq!(replica, 1);
        }
        other => panic!("expected an unfetchable replica to refuse the commit, got {other:?}"),
    }
    assert!(
        replicas_written(&counting, committed.generation()).is_empty(),
        "a replica that was merely unreachable is not known to be lost, so nothing \
         was written over it (spec: KL-13)",
    );
    assert!(
        !Library::read(store)
            .await
            .holds(&ControlObjectName::head(generation(2))),
        "and nothing of the batch was committed",
    );
}

/// KL-5, RV-7: a committed generation no replica of reads back is Keyring loss,
/// not a repair.
///
/// With no surviving valid replica there is nothing to copy, so this is not a
/// degraded set at all and the refusal is the one a read of it would give.
/// Saying otherwise would have a device report a repair it has no source for.
pub async fn a_keyring_no_replica_answers_stays_unreadable(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let first = commit_batch(request(store, index, &keys, adding(1)))
        .await
        .expect("a commit into a newly created Library must succeed");
    let committed = first.record.keyring().clone();

    for position in 0..committed.replica_count() {
        mangle_replica(store, &committed, position).await;
    }

    Library::upload_container(store, container_id(2)).await;
    let counting = CountingStore::around(store);
    let result = commit_batch(request(&counting, index, &keys, adding(2))).await;

    match result.map_err(|failure| *failure.error) {
        Err(CommitError::KeyringUnreadable {
            generation,
            cause: UnusableReplica::Unreadable(_),
            ..
        }) => assert_eq!(generation, committed.generation()),
        other => panic!("expected Keyring loss rather than a repair, got {other:?}"),
    }
    assert!(
        replicas_written(&counting, committed.generation()).is_empty(),
        "a generation with no valid replica has nothing to re-materialize from",
    );
    assert!(
        !Library::read(store)
            .await
            .holds(&ControlObjectName::head(generation(2))),
        "and nothing of the batch was committed",
    );
}

/// KL-9: replication is effective only within one generation: envelopes
/// introduced by a newer generation are protected only by that generation's
/// replicas, never by retained older generations.
///
/// Two generations are committed and every replica of the newer one is
/// damaged, while the older one is intact on Storage. The commit reads the
/// newer generation as Keyring loss: it does not fall back to the older set,
/// which does not hold the newer generation's envelopes, and it writes
/// nothing out of it.
pub async fn an_older_generation_does_not_stand_in_for_a_lost_newer_one(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let older = commit_batch(request(store, index, &keys, adding(1)))
        .await
        .expect("a commit into a newly created Library must succeed")
        .record
        .keyring()
        .clone();
    Library::upload_container(store, container_id(2)).await;
    let newer = commit_batch(request(store, index, &keys, adding(2)))
        .await
        .expect("the second commit must succeed")
        .record
        .keyring()
        .clone();

    for position in 0..newer.replica_count() {
        mangle_replica(store, &newer, position).await;
    }
    Library::read(store).await.keyring(store, &older).await;

    Library::upload_container(store, container_id(3)).await;
    let counting = CountingStore::around(store);
    let result = commit_batch(request(&counting, index, &keys, adding(3))).await;

    match result.map_err(|failure| *failure.error) {
        Err(CommitError::KeyringUnreadable {
            generation,
            cause: UnusableReplica::Unreadable(_),
            ..
        }) => assert_eq!(
            generation,
            newer.generation(),
            "the loss is the newer generation's, whatever the older one still holds",
        ),
        other => panic!("expected Keyring loss rather than a fallback, got {other:?}"),
    }
    for generation in [older.generation(), newer.generation()] {
        assert!(
            replicas_written(&counting, generation).is_empty(),
            "nothing is re-materialized from another generation",
        );
    }
    assert!(
        !Library::read(store)
            .await
            .holds(&ControlObjectName::head(generation(3))),
        "and nothing of the batch was committed",
    );
}
