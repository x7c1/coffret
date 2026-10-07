use super::commit_under_test::CommitUnderTest;
use super::fixtures::{control_keys, policy, prepared};
use super::refused_checkpoint::RefusedCheckpoint;
use crate::commit::{catch_up, commit_batch, CheckpointOutcome, CommitRequest, PreparedBatch};
use coffret_model::ContainerKind;

/// Repeated Snapshot failures leave all commits replayable. The threshold
/// triggers attempts but cannot bound history while writes fail (spec: CK-8,
/// CK-9, RV-1).
pub async fn repeated_checkpoint_failures_leave_the_journal_replayable(fixture: &CommitUnderTest) {
    let keys = control_keys();
    let refusing = RefusedCheckpoint {
        inner: fixture.store(),
    };
    let policy = policy().with_checkpoint_threshold(0);
    for seed in 1..=3 {
        let path = format!("books/{seed}.jpg");
        let batch = PreparedBatch::adding(vec![prepared(seed, ContainerKind::OneFile, &[&path])]);
        let outcome = commit_batch(
            CommitRequest::new(&refusing, fixture.index(), &keys, batch)
                .with_policy(policy.clone()),
        )
        .await
        .expect("checkpoint failure cannot undo a commit");
        assert!(matches!(
            outcome.checkpoint,
            CheckpointOutcome::Failed { .. }
        ));
    }
    catch_up(fixture.store(), fixture.other(), &keys, &policy.retry)
        .await
        .expect("a fresh device restores from the complete Journal without Snapshots");
    assert_eq!(fixture.other().entries_under(None).await.unwrap().len(), 3);
    assert_eq!(
        fixture.other().snapshot().await.unwrap(),
        fixture.index().snapshot().await.unwrap()
    );
}
