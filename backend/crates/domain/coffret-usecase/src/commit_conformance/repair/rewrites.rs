use crate::commit::commit_batch;
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::counting_store::CountingStore;
use crate::commit_conformance::fixtures::{
    container_id, control_keys, request, request_under, triplicate,
};
use crate::commit_conformance::library::{
    lose_replica, mangle_replica, misdigest_replica, replica_name, Library,
};

use super::{adding, one_repair, replicas_written};

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
/// replica carrying the one key table its digest binds (spec: KL-1, KL-2, FM-17);
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
    .expect("a commit into a newly created Library must succeed");
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
            repair.rewritten.iter().collect::<Vec<_>>(),
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
        // purpose key, all of them carrying one key table.
        Library::read(store).await.keyring(store, &committed).await;

        committed = outcome.record.keyring().clone();
    }
}

/// A replica that is there and cannot be read is replaced, both ways it can
/// happen (spec: KL-1, KL-5, KL-13).
///
/// The state that makes the walk cost what it costs. A position whose object has
/// gone is one a listing answers for free; a position whose object is *present*
/// and is not a replica of this generation can be found only by reading it, and
/// it leaves the set exactly as short. Both ways are here because they fail
/// different checks: bytes that will not open at all, and an object that opens,
/// authenticates, agrees with its name — and carries a key table its name does not
/// promise (spec: CP-10, FM-17).
pub async fn an_unreadable_replica_is_replaced(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let first = commit_batch(request(store, fixture.index(), &keys, adding(1)))
        .await
        .expect("a commit into a newly created Library must succeed");

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
        assert_eq!(repair.rewritten.iter().collect::<Vec<_>>(), vec![position]);

        Library::read(store).await.keyring(store, &committed).await;
        committed = outcome.record.keyring().clone();
    }
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
        .expect("a commit into a newly created Library must succeed");
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
