use coffret_model::ContainerKind;

use crate::commit::{commit_batch, PreparedBatch};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::fixtures::{
    container_id, control_keys, prepared, request, request_under, triplicate,
};
use crate::commit_conformance::library::Library;
use crate::commit_conformance::racing_store::RacingStore;
use crate::commit_conformance::watching_store::{Seen, WatchingStore};
use crate::progress::{Phase, Step};
use crate::recorded_progress::Recording;

/// A commit says how far it has got in the objects it stores, as each one is
/// stored.
///
/// Under three replicas the commit stores four objects — the candidate
/// Keyring's three replicas and then the head (spec: CP-8, KL-2, CP-2) — so it
/// walks `0/4` to `4/4`. The order is the protocol's and this only watches it:
/// every replica is on Storage before the head is created.
///
/// What makes the count worth having is that it moves while the commit is
/// still going, which the recording alone cannot show — a commit that said
/// every step at the end would record the same list. So the store notes what
/// the run had said each time an object was sent: the first replica goes out
/// with nothing stored yet, each one after it with the replicas before it
/// counted, and the head with every replica counted and itself not.
pub async fn a_commit_is_seen_part_way_in_the_objects_it_stores(fixture: &CommitUnderTest) {
    let keys = control_keys();
    Library::upload_container(fixture.store(), container_id(1)).await;

    let watching = Recording::default();
    let store = WatchingStore::around(fixture.store(), &watching);
    let batch = PreparedBatch::adding(vec![prepared(1, ContainerKind::Pack, &["books/p-1.png"])]);
    commit_batch(
        request_under(&store, fixture.index(), &keys, batch, triplicate()).watched_by(&watching),
    )
    .await
    .expect("a watched commit into an empty Library must succeed");

    let committing = |done| Some(Step::new(Phase::Committing, done, 4));
    assert_eq!(
        store.seen(),
        [
            Seen::Replica(committing(0)),
            Seen::Replica(committing(1)),
            Seen::Replica(committing(2)),
            Seen::Head(committing(3)),
        ],
        "each object goes out with the ones before it counted and itself not",
    );
    assert_eq!(
        watching.steps(),
        [0, 1, 2, 3, 4]
            .map(|done| Step::new(Phase::Committing, done, 4))
            .to_vec(),
        "a commit counts from nothing to every object it stored, one at a time",
    );
}

/// A commit that loses the head's slot counts again from nothing
/// (spec: CP-4, KL-3).
///
/// The candidate the lost attempt stored is not the one the rebase commits to:
/// the rebase writes a fresh generation over the new current set, and the
/// objects it stores are the ones still to do. A count that carried on from
/// where the lost attempt stopped would run past its total.
pub async fn a_commit_that_rebases_counts_again_from_nothing(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();
    Library::upload_container(store, container_id(1)).await;
    Library::upload_container(store, container_id(2)).await;

    let rival_batch =
        PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
    let racing = RacingStore::letting_in(store, fixture.other(), &keys, rival_batch);

    let watching = Recording::default();
    let batch = PreparedBatch::adding(vec![prepared(2, ContainerKind::OneFile, &["books/b.png"])]);
    let outcome =
        commit_batch(request(&racing, fixture.index(), &keys, batch).watched_by(&watching))
            .await
            .expect("losing the slot is a rebase, not a failure");
    assert_eq!(outcome.attempts, 2, "the first attempt lost the slot");

    // The suite's policy keeps two replicas, so three objects to an attempt.
    assert_eq!(
        watching.steps(),
        [0, 1, 2, 0, 1, 2, 3]
            .map(|done| Step::new(Phase::Committing, done, 3))
            .to_vec(),
        "the lost attempt reaches its head and no further; the rebase starts over",
    );
}
