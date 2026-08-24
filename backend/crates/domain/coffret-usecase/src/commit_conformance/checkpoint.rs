use coffret_model::{ContainerKind, Generation, SnapshotContent};

use crate::commit::{commit_batch, CheckpointOutcome, CommitRequest, PreparedBatch};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::faulty_store::FaultyStore;
use crate::commit_conformance::fixtures::{control_keys, policy, prepared, request};
use crate::commit_conformance::library::Library;
use crate::index::Index;
use crate::object_store::ObjectStore;

/// A threshold every commit crosses, so that a case can ask for a checkpoint
/// without committing sixty-four times to get one.
const ALWAYS_CHECKPOINT: u64 = 0;

/// A commit past the threshold writes the Snapshot of the head it became
/// (spec: CK-8, CK-10).
///
/// The Snapshot has to be the Index, not a summary of it: a device that adopts
/// it stands exactly where the writer stood (spec: CK-7, RV-1). So the case
/// compares what is on Storage against what the committing device's own Index
/// hands back — everything but `adopted_from`, which is that Index's own
/// provenance and is deliberately not carried by a Snapshot.
pub async fn a_checkpoint_is_written_once_the_threshold_is_crossed(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();

    let batch = PreparedBatch::adding(vec![
        prepared(1, ContainerKind::OneFile, &["albums/a.jpg"]),
        prepared(2, ContainerKind::Pack, &["books/p-1.png", "books/p-2.png"]),
    ]);
    let outcome = commit_batch(checkpointing(store, index, &keys, batch))
        .await
        .expect("a commit into an empty Library must succeed");

    match outcome.checkpoint {
        CheckpointOutcome::Written { ref object } => {
            assert_eq!(object.to_string(), "idx-0.cfrt")
        }
        ref other => panic!("expected this commit to write its checkpoint, got {other:?}"),
    }

    let held = index
        .snapshot()
        .await
        .expect("a committed device has something to checkpoint");
    let stored = Library::read(store)
        .await
        .snapshot(store, Generation::FIRST)
        .await;
    assert_eq!(
        stored,
        SnapshotContent {
            adopted_from: None,
            ..held
        },
        "the Snapshot on Storage is this device's Index (spec: CK-7)",
    );
    assert_eq!(stored.checkpoint.head_generation, Generation::FIRST);
    assert_eq!(stored.checkpoint.keyring, outcome.record.keyring);
}

/// Below the threshold no Snapshot is written (spec: CK-8).
///
/// A commit pays for its own batch and never for the whole Library's Index, so
/// the absence is the rule rather than an optimization: the records this commit
/// added stay replayable, and the next qualifying moment writes the Snapshot
/// that covers them.
pub async fn no_checkpoint_is_written_below_the_threshold(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let keys = control_keys();

    let batch = PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
    let outcome = commit_batch(request(store, fixture.index(), &keys, batch))
        .await
        .expect("a commit into an empty Library must succeed");

    assert!(
        matches!(outcome.checkpoint, CheckpointOutcome::NotDue),
        "expected no checkpoint below the threshold, got {:?}",
        outcome.checkpoint,
    );
    assert!(
        !Library::read(store).await.holds_any("idx-"),
        "nothing was written into the head's snapshot slot",
    );
}

/// Two writers racing for one snapshot slot converge on one checkpoint
/// (spec: CK-11).
///
/// Losing that conditional create is not a failure and is not retried under
/// another name: two Snapshots of one head would be the same checkpoint, and a
/// second name for one head would leave readers two to choose between. So the
/// loser reads the slot back, finds a valid Snapshot of the head it was
/// checkpointing, and is done.
pub async fn a_snapshot_slot_taken_by_a_sibling_converges(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let sibling = FaultyStore::losing_the_snapshot_slot(store);
    let keys = control_keys();

    let batch = PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
    let outcome = commit_batch(checkpointing(&sibling, index, &keys, batch))
        .await
        .expect("losing the snapshot slot does not fail the commit");

    match outcome.checkpoint {
        CheckpointOutcome::Existing { ref object } => {
            assert_eq!(object.to_string(), "idx-0.cfrt")
        }
        ref other => panic!("expected the sibling's checkpoint to settle it, got {other:?}"),
    }

    let library = Library::read(store).await;
    assert_eq!(
        library
            .names()
            .iter()
            .filter(|name| name.starts_with("idx-"))
            .count(),
        1,
        "one head has one checkpoint",
    );
    let stored = library.snapshot(store, Generation::FIRST).await;
    assert_eq!(stored.checkpoint.head_generation, Generation::FIRST);
}

/// A commit under a policy that checkpoints every head.
fn checkpointing<'a>(
    store: &'a dyn ObjectStore,
    index: &'a dyn Index,
    keys: &'a crate::commit::ControlKeys,
    batch: PreparedBatch,
) -> CommitRequest<'a> {
    CommitRequest::new(store, index, keys, batch)
        .with_policy(policy().with_checkpoint_threshold(ALWAYS_CHECKPOINT))
}
