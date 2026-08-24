use coffret_model::{ContainerKind, Generation};

use crate::commit::{commit_batch, CommitOutcome, PreparedBatch};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::fixtures::{container_id, control_keys, path, prepared, request};
use crate::commit_conformance::library::{mapped, Library};
use crate::commit_conformance::racing_store::RacingStore;

/// Of two writers starting from one head, one commits and the other rebases
/// (spec: CP-3, CP-4, CK-9).
///
/// This is the rule the whole protocol rests on, and it is the one that cannot
/// be checked from one side: both devices reserve the same slot, exactly one
/// conditional create succeeds, and the refused one has committed nothing. What
/// it does next is the second half — it catches up onto the head the winner
/// created, prepares a fresh Keyring generation over the new current set, and
/// commits the generation after the winner's, naming it as its predecessor.
///
/// Which of the two wins is not asserted, because nothing in the protocol says:
/// a writer that starts late enough finds the head already there and never
/// races at all. What is asserted holds either way — one head chain, no gap in
/// it, and the later record built on the earlier one.
pub async fn two_writers_settle_on_one_head_chain(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    Library::upload_container(store, container_id(2)).await;

    let first = PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
    let second = PreparedBatch::adding(vec![prepared(2, ContainerKind::OneFile, &["books/b.png"])]);

    let (left, right) = tokio::join!(
        commit_batch(request(store, fixture.index(), &keys, first)),
        commit_batch(request(store, fixture.other(), &keys, second)),
    );
    let left = left.expect("one writer commits and the other rebases; neither fails");
    let right = right.expect("one writer commits and the other rebases; neither fails");

    let (winner, loser): (CommitOutcome, CommitOutcome) =
        if left.record.generation < right.record.generation {
            (left, right)
        } else {
            (right, left)
        };

    assert_eq!(winner.record.generation, Generation::FIRST);
    assert_eq!(winner.record.prev, None);
    assert_eq!(
        loser.record.generation,
        Generation::new(1),
        "the rebased commit takes the generation after the one it found",
    );
    assert_eq!(
        loser.record.prev,
        Some(winner.record.generation),
        "the rebased commit names the head it was built on (spec: CP-2, FM-15)",
    );

    // Two commits, two Keyring generations: the candidate the loser prepared
    // before it lost stays uncommitted and is not the one it committed
    // (spec: KL-3).
    assert_eq!(winner.record.keyring.generation(), Generation::FIRST);
    assert_eq!(loser.record.keyring.generation(), Generation::new(1));

    let library = Library::read(store).await;
    assert_eq!(
        library.record(store, Generation::FIRST).await,
        winner.record
    );
    assert_eq!(
        library.record(store, Generation::new(1)).await,
        loser.record
    );

    let keyring = library.keyring(store, &loser.record.keyring).await;
    assert_eq!(
        mapped(&keyring),
        vec![container_id(1), container_id(2)],
        "the committed Keyring maps what both commits left current (spec: KL-7)",
    );
}

/// A writer whose slot was consumed rebases onto the new head and commits
/// (spec: CP-4, CK-9, EP-7).
///
/// The case above starts two writers together, which is how it happens in life
/// but not how it can be asserted: whether they collide at all depends on the
/// runtime and on how fast Storage answers. This one puts the collision exactly
/// where the rule is — the rival's record lands in the slot while this writer is
/// spending it — so the rebase is exercised on every backend rather than on the
/// slow ones.
///
/// What the rebase costs is the second attempt doing the whole flow again
/// against the head it found: catching up, re-checking the Entry Paths, and
/// preparing a Keyring generation over the new current set. The candidate it
/// built before it lost stays where it is and selects nothing (spec: KL-3).
pub async fn a_writer_that_loses_the_slot_rebases_onto_the_new_head(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    Library::upload_container(store, container_id(2)).await;

    let rival_batch =
        PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
    let racing = RacingStore::letting_in(store, fixture.other(), &keys, rival_batch);

    let batch = PreparedBatch::adding(vec![prepared(2, ContainerKind::OneFile, &["books/b.png"])]);
    let outcome = commit_batch(request(&racing, fixture.index(), &keys, batch))
        .await
        .expect("losing the slot is a rebase, not a failure");

    assert_eq!(outcome.attempts, 2, "the first attempt lost the slot");
    assert_eq!(outcome.record.generation, Generation::new(1));
    assert_eq!(
        outcome.record.prev,
        Some(Generation::FIRST),
        "the rebased record names the head the rival committed",
    );
    assert_eq!(
        outcome.record.keyring.generation(),
        Generation::new(1),
        "the rebase prepares a fresh generation rather than reusing the candidate \
         it had already written (spec: KL-3)",
    );

    let library = Library::read(store).await;
    let rival = library.record(store, Generation::FIRST).await;
    assert_eq!(rival.additions.len(), 1);
    assert_eq!(rival.additions[0].container.id, container_id(1));

    let keyring = library.keyring(store, &outcome.record.keyring).await;
    assert_eq!(
        mapped(&keyring),
        vec![container_id(1), container_id(2)],
        "the rebased Keyring covers what the rival left current as well (spec: CP-8)",
    );

    let index = fixture.index();
    for text in ["albums/a.jpg", "books/b.png"] {
        assert!(
            index
                .entry_at(&path(text))
                .await
                .expect("asking the Index for a path must succeed")
                .is_some(),
            "the rebased device catalogs both commits",
        );
    }
}
