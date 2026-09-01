use coffret_model::{ContainerKind, Generation, JournalRecord};

use crate::commit::{catch_up, commit_batch, CommitError, CommitOutcome, PreparedBatch};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::fixtures::{
    container_id, control_keys, path, policy, prepared, request,
};
use crate::commit_conformance::library::{mapped, Library};
use crate::commit_conformance::racing_store::RacingStore;
use crate::commit_conformance::rival_index::RivalIndex;
use crate::index::Index;
use crate::index_error::IndexError;
use crate::object_store::ObjectStore;

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

/// Two catch-ups replaying one catalog converge rather than one of them failing
/// (spec: CK-9, CP-1).
///
/// A catalog is held by every process that has the Library open, and each of
/// them lists the Journal from its own reading of the checkpoint — so a server
/// answering a browser and a `sync` in a terminal reach the same records. The
/// Index refuses the second application of a record and must: one Entry Path
/// admits one current Entry (spec: EP-5). What the refusal means is the
/// replay's to decide, and the checkpoint is what decides it — a catalog
/// standing past the record says somebody applied it, which is the outcome this
/// device wanted anyway.
///
/// The rival lands two records while this device is applying the first of them,
/// so the case covers both halves of the convergence: the refusal itself, and
/// the records after it that the moved checkpoint already covers.
pub async fn two_replays_of_one_catalog_converge(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();
    let committed = three_heads(store, fixture.other(), &keys).await;

    // This device has seen none of it, and a rival replayer gets in ahead of it
    // at the second record, carrying the catalog all the way to the head.
    let contended = RivalIndex::racing(fixture.index(), committed[1..].to_vec());
    catch_up(store, &contended, &keys, &policy().retry)
        .await
        .expect("a record another replayer applied first is not a failure");

    let caught_up = fixture
        .index()
        .snapshot()
        .await
        .expect("a caught-up catalog has a state to checkpoint");
    assert_eq!(
        caught_up.checkpoint.head_generation,
        Generation::new(2),
        "the contended replay reaches the head, once, and does not go back",
    );
    assert_eq!(
        caught_up,
        fixture
            .other()
            .snapshot()
            .await
            .expect("the committing device has a state to checkpoint"),
        "a contended replay lands where an uncontended one does",
    );
}

/// A refused replay nothing explains stops the catch-up (spec: EP-5, EP-6).
///
/// The other half of the rule above, and the reason it is a checkpoint reading
/// and not a swallowed error: a duplicate over a catalog that stands exactly
/// where it did is a Library state no commit could have produced, and a replay
/// that stepped over it would carry that state forward under the name of a race
/// that never happened.
pub async fn a_refused_replay_no_checkpoint_explains_is_reported(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    let batch = PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
    commit_batch(request(store, fixture.other(), &keys, batch))
        .await
        .expect("committing into an empty Library must succeed");

    let refusing = RivalIndex::refusing_without_moving(fixture.index(), Generation::FIRST);
    match catch_up(store, &refusing, &keys, &policy().retry).await {
        Err(CommitError::Index(IndexError::DuplicatePath { path })) => {
            assert_eq!(path.as_str(), "albums/a.jpg")
        }
        Err(other) => panic!("expected the refusal to be reported as it stands, got {other:?}"),
        Ok(_) => panic!("a refusal nothing explains is not a replay that converged"),
    }

    assert!(
        fixture
            .index()
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .is_none(),
        "the catalog stands where it did, and the caller has been told why",
    );
}

/// Three heads in the Library, committed by the other device, as the records
/// they are on Storage.
async fn three_heads(
    store: &dyn ObjectStore,
    committer: &dyn Index,
    keys: &crate::commit::ControlKeys,
) -> Vec<JournalRecord> {
    let mut records = Vec::with_capacity(3);
    for (seed, text) in [(1u8, "albums/a.jpg"), (2, "books/b.png"), (3, "notes.txt")] {
        Library::upload_container(store, container_id(seed)).await;
        let batch = PreparedBatch::adding(vec![prepared(seed, ContainerKind::OneFile, &[text])]);
        records.push(
            commit_batch(request(store, committer, keys, batch))
                .await
                .expect("committing into the Library must succeed")
                .record,
        );
    }
    records
}
