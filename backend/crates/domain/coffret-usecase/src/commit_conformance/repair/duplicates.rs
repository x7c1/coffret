use crate::commit::{commit_batch, read_committed, CommitOutcome, ControlListing};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::counting_store::CountingStore;
use crate::commit_conformance::fixtures::{container_id, control_keys, policy, request};
use crate::commit_conformance::library::{lose_replica, replica_name, Library};
use crate::commit_conformance::twinned_store::{Placement, TwinnedStore};
use crate::generations::generation;

use super::{adding, replicas_written};

/// A position whose name carries a corrupt copy and a valid copy reads as the
/// valid one, whichever the listing gives last (spec: KL-1, KL-14).
///
/// What a repair on a Storage whose names are not unique leaves behind: the
/// valid copy it wrote beside the one that did not read back valid, under one
/// name. Either copy that reads back valid is the replica (spec: KL-14), so the
/// position is valid, and a commit over it repairs nothing and writes nothing
/// to it. A reader that kept one copy per name would leave the verdict to the
/// provider's listing order: where the corrupt copy came last it would find the
/// position lost on every commit and repair it again, one more duplicate each
/// time. Both orders are run, so the case holds whatever order the store under
/// test lists in.
///
/// The commit reads the set through its examination; every other flow — a
/// fetch, a freeze, a delete — through the read of the committed Keyring, which
/// is read here too, against the same listing, and must not have had to step
/// over the position.
pub async fn a_replica_listed_twice_reads_as_its_valid_copy(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();
    let policy = policy();

    Library::upload_container(store, container_id(1)).await;
    let first = commit_batch(request(store, fixture.index(), &keys, adding(1)))
        .await
        .expect("a commit into a newly created Library must succeed");

    let mut committed = first.record.keyring().clone();
    for (seed, placement) in [(2u8, Placement::Last), (3, Placement::First)] {
        let twinned = TwinnedStore::beside(
            store,
            &replica_name(&committed, 0),
            b"not a Keyring replica at all".to_vec(),
            placement,
        );
        assert_eq!(
            Library::read(&twinned)
                .await
                .count(&replica_name(&committed, 0)),
            2,
            "the position's name carries two objects, which is what the case is about",
        );

        let listing = ControlListing::read(&twinned, &policy.retry)
            .await
            .expect("listing the store must succeed");
        let read = read_committed(&twinned, &keys, &policy.retry, &listing, &committed)
            .await
            .unwrap_or_else(|error| {
                panic!("a set with a valid copy at every position reads ({placement:?}): {error}")
            });
        assert!(
            read.short.is_none(),
            "a valid copy makes the position valid, so the read stepped over nothing \
             ({placement:?})",
        );

        Library::upload_container(store, container_id(seed)).await;
        let counting = CountingStore::around(&twinned);
        let outcome = commit_batch(request(&counting, fixture.index(), &keys, adding(seed)))
            .await
            .expect("a commit over a set with a valid copy at every position commits");

        assert!(
            outcome.repairs.is_empty(),
            "a position with a valid copy is not repaired again ({placement:?}), got {:?}",
            outcome.repairs,
        );
        assert!(
            replicas_written(&counting, committed.generation()).is_empty(),
            "nothing of the committed set was rewritten ({placement:?})",
        );

        committed = outcome.record.keyring().clone();
    }
}

/// Two devices meeting one degraded set both commit (spec: KL-14, CP-4).
///
/// Repair is an unconditional write onto a name whose content is fixed: a
/// replica at `(generation, set_digest, index)` has exactly one valid content,
/// so two devices that both decide to rewrite it write the same key table under
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
        .expect("a commit into a newly created Library must succeed");
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
    assert_eq!(winner.record.generation(), generation(2));
    assert_eq!(
        loser.record.prev(),
        Some(winner.record.generation()),
        "the rebased commit was built on the head the other committed (spec: CP-4)",
    );

    Library::read(store).await.keyring(store, &committed).await;
}
