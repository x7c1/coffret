use coffret_model::ControlObjectName;

use crate::commit::{commit_batch, CommitError};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::faulty_store::FaultyStore;
use crate::commit_conformance::fixtures::{
    container_id, control_keys, policy, request, request_under,
};
use crate::commit_conformance::library::{lose_replica, replica_name, Library};
use crate::commit_conformance::racing_store::RacingStore;
use crate::error::Error;
use crate::generations::generation;

use super::{adding, one_repair, one_repair_of};

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
        .expect("a commit into a newly created Library must succeed");
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
        repair.rewritten.iter().collect::<Vec<_>>(),
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

/// A repair performed before the rebases ran out is still reported, on the
/// failure (spec: KL-15, CP-4).
///
/// Running out of attempts fails the run, and it fails it after the attempt
/// that lost the slot had already put a replica back. That replica stands on
/// Storage whatever became of the batch, so a run that reported repairs only
/// when it committed would have written to the Library and told nobody.
pub async fn a_repair_before_the_rebases_ran_out_is_still_reported(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let first = commit_batch(request(store, fixture.index(), &keys, adding(1)))
        .await
        .expect("a commit into a newly created Library must succeed");
    let committed = first.record.keyring().clone();

    lose_replica(store, &committed, 1).await;
    for seed in [2, 3] {
        Library::upload_container(store, container_id(seed)).await;
    }

    // One attempt, and a rival that takes the slot the moment this device
    // reaches the create of its record — after the attempt's repair.
    let racing = RacingStore::letting_in(store, fixture.other(), &keys, adding(3));
    let failure = commit_batch(request_under(
        &racing,
        fixture.index(),
        &keys,
        adding(2),
        policy().with_attempts(1),
    ))
    .await
    .expect_err("an attempt that lost the slot with none left fails the run");

    assert!(
        matches!(
            *failure.error,
            CommitError::ConflictLimitReached { attempts: 1 }
        ),
        "the run ran out of attempts, got {:?}",
        failure.error,
    );
    let repair = one_repair_of(&failure.repairs);
    assert_eq!(repair.generation, committed.generation());
    assert_eq!(
        repair.rewritten.iter().collect::<Vec<_>>(),
        vec![1],
        "the failure names the position the attempt put back (spec: KL-15)",
    );
    assert!(
        Library::read(store)
            .await
            .holds(&replica_name(&committed, 1)),
        "which stands on Storage although the run committed nothing (spec: KL-14)",
    );
}

/// A repair performed before a later step failed is still reported, on the
/// failure (spec: KL-15, CP-1).
///
/// The examination completed and the set is whole again; what failed was the
/// create of the Journal record, after it. The batch committed nothing, and the
/// repair is still one the run performed.
pub async fn a_repair_before_a_refused_record_is_still_reported(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let first = commit_batch(request(store, fixture.index(), &keys, adding(1)))
        .await
        .expect("a commit into a newly created Library must succeed");
    let committed = first.record.keyring().clone();

    lose_replica(store, &committed, 0).await;
    Library::upload_container(store, container_id(2)).await;

    let refusing = FaultyStore::refusing_the_head(store);
    let failure = commit_batch(request(&refusing, fixture.index(), &keys, adding(2)))
        .await
        .expect_err("a refused record fails the run");

    assert!(
        matches!(*failure.error, CommitError::Storage(Error::Rejected { .. })),
        "the create of the record was refused, got {:?}",
        failure.error,
    );
    let repair = one_repair_of(&failure.repairs);
    assert_eq!(repair.generation, committed.generation());
    assert_eq!(
        repair.rewritten.iter().collect::<Vec<_>>(),
        vec![0],
        "the failure names the position the examination put back (spec: KL-15)",
    );
    let library = Library::read(store).await;
    assert!(
        !library.holds(&ControlObjectName::head(generation(2))),
        "nothing of the batch was committed (spec: CP-1)",
    );
    library.keyring(store, &committed).await;
}
