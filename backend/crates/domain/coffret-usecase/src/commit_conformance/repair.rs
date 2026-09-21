use coffret_model::{ContainerKind, ControlObjectName, Generation};

use crate::commit::{
    commit_batch, CommitError, CommitOutcome, InvalidReplica, KeyringRepair, PreparedBatch,
    UnrepairedReplica,
};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::counting_store::CountingStore;
use crate::commit_conformance::faulty_store::FaultyStore;
use crate::commit_conformance::fixtures::{
    container_id, control_keys, prepared, request, request_under, triplicate,
};
use crate::commit_conformance::library::{
    lose_replica, mangle_replica, misdigest_replica, replica_name, Library,
};
use crate::commit_conformance::racing_store::RacingStore;
use crate::error::Error;
use crate::generations::generation;

/// A committed set that has lost a replica is complete again after the next
/// commit (spec: KL-11, KL-13, KL-14).
///
/// Three losses in one Library, at the first position, a middle one, and the
/// last, because the last is the one a walk that stops at the first valid
/// replica can never see: it would read position zero, be satisfied, and commit
/// straight over a set that has been one short for months. That is the whole
/// reason the examination is a full walk.
///
/// What each round asserts is three things together. The commit reported which
/// positions it rewrote, so a caller can surface them (spec: KL-15); the
/// committed set reads back complete and valid to a device with no Index, every
/// replica carrying the one mapping its digest binds (spec: KL-1, KL-2, FM-17);
/// and exactly one object was written for it, so the positions that were already
/// valid were left alone — repair re-materializes what is missing and never
/// rewrites the Library (spec: KL-13).
pub async fn a_lost_replica_is_rewritten_before_the_next_commit(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let first = commit_batch(request_under(
        store,
        fixture.index(),
        &keys,
        adding(1),
        triplicate(),
    ))
    .await
    .expect("a commit into an empty Library must succeed");
    assert!(
        first.repairs.is_empty(),
        "the Library's first commit had no committed Keyring to repair (spec: FM-13)",
    );

    let mut committed = first.record.keyring().clone();
    for (seed, position) in [(2u8, 0u16), (3, 1), (4, 2)] {
        lose_replica(store, &committed, position).await;

        Library::upload_container(store, container_id(seed)).await;
        let counting = CountingStore::around(store);
        let outcome = commit_batch(request_under(
            &counting,
            fixture.index(),
            &keys,
            adding(seed),
            triplicate(),
        ))
        .await
        .expect("a commit over a degraded committed Keyring repairs it and commits");

        let repair = one_repair(&outcome);
        assert_eq!(repair.generation, committed.generation());
        assert_eq!(
            repair.rewritten,
            vec![position],
            "the outcome names the position that was rewritten (spec: KL-15)",
        );
        assert_eq!(
            replicas_written(&counting, committed.generation()),
            vec![replica_name(&committed, position).to_string()],
            "exactly the lost position was written; a valid replica is never rewritten",
        );

        // Complete and valid again, read the way a device with no Index reads
        // it: every declared position present, each opening under the Keyring
        // purpose key, all of them carrying one mapping.
        Library::read(store).await.keyring(store, &committed).await;

        committed = outcome.record.keyring().clone();
    }
}

/// A replica that is there and cannot be read is replaced, both ways it can
/// happen (spec: KL-1, KL-5, KL-13).
///
/// The state that makes the walk cost what it costs. A position whose object has
/// gone is one a listing settles for free; a position whose object is *present*
/// and is not a replica of this generation can be found only by reading it, and
/// it leaves the set exactly as short. Both ways are here because they fail
/// different checks: bytes that will not open at all, and an object that opens,
/// authenticates, agrees with its name — and carries a mapping its name does not
/// promise (spec: CP-10, FM-17).
pub async fn an_unreadable_replica_is_replaced(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let first = commit_batch(request(store, fixture.index(), &keys, adding(1)))
        .await
        .expect("a commit into an empty Library must succeed");

    let mut committed = first.record.keyring().clone();
    for (seed, position, mangle) in [(2u8, 1u16, true), (3, 0, false)] {
        match mangle {
            true => mangle_replica(store, &committed, position).await,
            false => misdigest_replica(store, &committed, position).await,
        }
        assert!(
            Library::read(store)
                .await
                .holds(&replica_name(&committed, position)),
            "the position is occupied, which is what makes it worth reading",
        );

        Library::upload_container(store, container_id(seed)).await;
        let outcome = commit_batch(request(store, fixture.index(), &keys, adding(seed)))
            .await
            .expect("a commit over an unreadable replica repairs it and commits");

        let repair = one_repair(&outcome);
        assert_eq!(repair.generation, committed.generation());
        assert_eq!(repair.rewritten, vec![position]);

        Library::read(store).await.keyring(store, &committed).await;
        committed = outcome.record.keyring().clone();
    }
}

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
        .expect("a commit into an empty Library must succeed");
    let committed = first.record.keyring().clone();

    lose_replica(store, &committed, 1).await;
    Library::upload_container(store, container_id(2)).await;

    let refusing = FaultyStore::refusing_replica_writes(store, 1);
    let counting = CountingStore::around(&refusing);
    let result = commit_batch(request(&counting, index, &keys, adding(2))).await;

    match result {
        Err(CommitError::UnrepairedKeyring {
            generation,
            ref needed,
            ref rewritten,
            replica,
            cause: UnrepairedReplica::Unwritten(ref error),
        }) => {
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
        !refused.holds(&ControlObjectName::head(generation(1))),
        "nothing of the batch was committed (spec: KL-16)",
    );
    assert!(
        replicas_written(&counting, generation(1)).is_empty(),
        "and the batch's own Keyring candidate was never written either",
    );
    assert_eq!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .expect("the first commit left one")
            .head_generation(),
        Generation::FIRST,
        "the Index stands where the first commit left it (spec: CP-1)",
    );

    // The next run, with the provider taking writes again.
    let outcome = commit_batch(request(store, index, &keys, adding(2)))
        .await
        .expect("a later run repairs the set and commits the same batch");
    assert_eq!(one_repair(&outcome).rewritten, vec![1]);
    assert_eq!(outcome.record.generation(), generation(1));
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
        .expect("a commit into an empty Library must succeed");
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

    match result {
        Err(CommitError::UnrepairedKeyring {
            generation,
            ref needed,
            ref rewritten,
            replica,
            cause: UnrepairedReplica::Unwritten(_),
        }) => {
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
        !library.holds(&ControlObjectName::head(generation(1))),
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
        .expect("a commit into an empty Library must succeed");
    let committed = first.record.keyring().clone();

    let hidden = Library::read(store)
        .await
        .handle(&replica_name(&committed, 1));
    Library::upload_container(store, container_id(2)).await;

    let hiding = FaultyStore::hiding(store, &hidden);
    let counting = CountingStore::around(&hiding);
    let result = commit_batch(request(&counting, index, &keys, adding(2))).await;

    match result {
        Err(CommitError::UnrepairedKeyring {
            generation,
            ref needed,
            ref rewritten,
            replica,
            cause: UnrepairedReplica::Unfetchable(_),
        }) => {
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
            .holds(&ControlObjectName::head(generation(1))),
        "and nothing of the batch was committed",
    );
}

/// A committed generation no replica of reads back is Keyring loss, not a
/// repair (spec: KL-5, RV-7).
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
        .expect("a commit into an empty Library must succeed");
    let committed = first.record.keyring().clone();

    for position in 0..committed.replica_count() {
        mangle_replica(store, &committed, position).await;
    }

    Library::upload_container(store, container_id(2)).await;
    let counting = CountingStore::around(store);
    let result = commit_batch(request(&counting, index, &keys, adding(2))).await;

    match result {
        Err(CommitError::KeyringUnreadable {
            generation,
            cause: InvalidReplica::Unreadable(_),
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
            .holds(&ControlObjectName::head(generation(1))),
        "and nothing of the batch was committed",
    );
}

/// A complete committed set costs no writes and reports no repair (spec: KL-13,
/// KL-15).
///
/// The ordinary case, and the one that says what the full walk is allowed to
/// cost: reads, and not a byte of writing. A run that put no replica back
/// reports no repair at all, which is what lets a shell stay silent about the
/// Keyring on a healthy run — the news KL-15 asks for is a loss and the repair
/// it called for, and there was neither. That the walk did look is what the
/// cases above hold: a set short one position comes back complete.
pub async fn a_complete_set_costs_no_writes_and_reports_no_repair(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let first = commit_batch(request(store, fixture.index(), &keys, adding(1)))
        .await
        .expect("a commit into an empty Library must succeed");
    let committed = first.record.keyring().clone();

    Library::upload_container(store, container_id(2)).await;
    let counting = CountingStore::around(store);
    let outcome = commit_batch(request(&counting, fixture.index(), &keys, adding(2)))
        .await
        .expect("a commit over a complete committed Keyring must succeed");

    assert!(
        outcome.repairs.is_empty(),
        "a complete set leaves the run with no repair to report, got {:?}",
        outcome.repairs,
    );
    assert!(
        replicas_written(&counting, committed.generation()).is_empty(),
        "and nothing of the committed generation was written",
    );
}

/// A repair an attempt performed before losing the commit slot is still the
/// run's to report (spec: KL-15, CP-4).
///
/// Losing the slot is a rebase rather than a failure, and the attempt that
/// follows examines whatever set the winner committed — a set this device has
/// no repair to perform on, the generation it did repair having been left
/// behind with the attempt that lost. So a run that reported only its last
/// examination would put a replica back and then tell the person who asked for
/// it that their Library had lost nothing.
pub async fn a_repair_before_a_lost_slot_is_still_reported(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let first = commit_batch(request(store, fixture.index(), &keys, adding(1)))
        .await
        .expect("a commit into an empty Library must succeed");
    let committed = first.record.keyring().clone();

    lose_replica(store, &committed, 1).await;
    for seed in [2, 3] {
        Library::upload_container(store, container_id(seed)).await;
    }

    // The rival takes the slot at the moment this device reaches the create of
    // its record, which is long after the repair the attempt has by then
    // performed — so what the rebase meets is a committed set this run made
    // complete, and there is nothing left there for it to report.
    let racing = RacingStore::letting_in(store, fixture.other(), &keys, adding(3));
    let outcome = commit_batch(request(&racing, fixture.index(), &keys, adding(2)))
        .await
        .expect("losing the slot is a rebase, not a failure");

    assert_eq!(outcome.attempts, 2, "the first attempt lost the slot");
    let repair = one_repair(&outcome);
    assert_eq!(
        repair.generation,
        committed.generation(),
        "the repair names the generation the attempt that lost examined",
    );
    assert_eq!(
        repair.rewritten,
        vec![1],
        "and the position that attempt put back (spec: KL-15)",
    );
    assert!(
        Library::read(store)
            .await
            .holds(&replica_name(&committed, 1)),
        "which stands on Storage whichever device won the slot (spec: KL-14)",
    );
}

/// Two devices meeting one degraded set both commit (spec: KL-14, CP-4).
///
/// Repair is an unconditional write onto a name whose content is fixed: a
/// replica at `(generation, set_digest, index)` has exactly one valid content,
/// so two devices that both decide to rewrite it write the same mapping under
/// the same digest. There is no reservation to lose and no loser to report — a
/// duplicate is benign, and the objects differing in the nonce each was sealed
/// with changes nothing the digest covers.
///
/// Which of the two writes the position, or whether both do, depends on how the
/// runtime interleaves them and is deliberately not asserted. What holds either
/// way is that neither is refused for what the other did, one commit rebases
/// onto the other's head (spec: CP-4), and the set both of them examined is
/// complete afterwards.
pub async fn two_devices_repairing_one_position_both_commit(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let first = commit_batch(request(store, fixture.index(), &keys, adding(1)))
        .await
        .expect("a commit into an empty Library must succeed");
    let committed = first.record.keyring().clone();

    lose_replica(store, &committed, 1).await;
    for seed in [2, 3] {
        Library::upload_container(store, container_id(seed)).await;
    }

    let (left, right) = tokio::join!(
        commit_batch(request(store, fixture.index(), &keys, adding(2))),
        commit_batch(request(store, fixture.other(), &keys, adding(3))),
    );
    let left: CommitOutcome = left.expect("neither device is refused for what the other repaired");
    let right: CommitOutcome =
        right.expect("neither device is refused for what the other repaired");

    let (winner, loser) = match left.record.generation() < right.record.generation() {
        true => (left, right),
        false => (right, left),
    };
    assert_eq!(
        winner.record.generation(),
        Generation::FIRST.next().unwrap()
    );
    assert_eq!(
        loser.record.prev(),
        Some(winner.record.generation()),
        "the rebased commit was built on the head the other committed (spec: CP-4)",
    );

    Library::read(store).await.keyring(store, &committed).await;
}

/// The one repair a run performed, or a panic saying what it reported instead.
///
/// A run carries a repair per attempt that put a position back, so "one repair"
/// is an assertion of its own: a case that read the first entry of the list
/// would pass just as well for a run that repaired the same set twice.
fn one_repair(outcome: &CommitOutcome) -> &KeyringRepair {
    match &outcome.repairs[..] {
        [repair] => repair,
        other => panic!("expected the run to report one repair, got {other:?}"),
    }
}

/// The batch each round of these cases commits.
///
/// One Container at a path of its own, so that no round is refused for an Entry
/// Path another round already claimed (spec: EP-6) and every commit has
/// something to commit.
fn adding(seed: u8) -> PreparedBatch {
    let path = format!("albums/{seed}.jpg");
    PreparedBatch::adding(vec![prepared(seed, ContainerKind::OneFile, &[&path])])
}

/// Every Keyring replica of one generation the run wrote, in the order it wrote
/// them.
///
/// The names rather than a count, because "which position" is the whole of what
/// a repair case is about: a run that rewrote the valid replicas alongside the
/// lost one would answer a count exactly as the right run does.
fn replicas_written(counting: &CountingStore<'_>, generation: Generation) -> Vec<String> {
    counting
        .written()
        .into_iter()
        .filter(|name| is_replica_of(name, generation))
        .collect()
}

/// Whether a name is a Keyring replica of one generation (spec: FM-12).
fn is_replica_of(name: &str, generation: Generation) -> bool {
    matches!(
        ControlObjectName::parse(name),
        Ok(ControlObjectName::KeyringReplica { generation: found, .. }) if found == generation
    )
}
