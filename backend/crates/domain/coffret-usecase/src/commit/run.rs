use tracing::debug;

use crate::commit::commit_error::{CommitError, CommitResult};
use crate::commit::commit_outcome::CommitOutcome;
use crate::commit::commit_request::CommitRequest;
use crate::commit::journal::Attempted;
use crate::commit::{candidate, catch_up, journal, keyring, settle};
use crate::committed_batch::CommittedBatch;

/// Makes a prepared batch the Library's next committed state.
///
/// The whole flow, in the order the commit protocol fixes: catch the Index up
/// to the current head (spec: CK-9), refuse the batch if its Entry Paths would
/// collide (spec: EP-6), write and verify the Keyring generation the commit will
/// select (spec: CP-8, KL-2), and spend the head's commit slot on the Journal
/// record (spec: CP-2, CP-3). Creating that object is the commit point: before
/// it the batch has changed nothing, and after it the batch's additions and
/// removals are part of the current Container set, never partially (spec: CP-1).
///
/// Losing the slot is a normal outcome and not an error. The attempt rebases —
/// the same catch-up, the same uniqueness check, a fresh Keyring generation over
/// the new current set — and tries again, up to
/// [`CommitPolicy::attempts`](super::CommitPolicy::attempts). Nothing is ever
/// resolved by comparing timestamps (spec: CP-4, CP-7, EP-7).
///
/// What happens after the record exists cannot un-commit it (spec: CP-1).
/// Trashing the removed Containers and writing the head's Snapshot are both
/// retryable later and neither failing fails the call: [`CommitOutcome`] reports
/// what of the two did not finish (spec: CK-8).
///
/// Refreshing the Index is the one post-commit step that does fail the call, and
/// it fails it with the batch committed. A later catch-up replays the record and
/// restores the Library-wide half of that refresh, but the device-local half —
/// which files this device put on disk (spec: EP-10) and which spools stop being
/// pending (spec: OC-2) — is known here and nowhere else, so it is not something
/// to report and carry on from. The caller that meets this error is stale rather
/// than uncommitted: offering the same batch again would refuse it as an Entry
/// Path collision with its own Containers, which the replay has by then made
/// current (spec: EP-6).
///
/// The Keyring replicas of an attempt that then lost the race stay on Storage as
/// an uncommitted candidate. That is what they are meant to be: a candidate set
/// selects nothing until a commit names its exact tuple (spec: KL-3), and
/// disposing of one is orphan cleanup's business (spec: KL-12, OC-2).
pub async fn commit_batch(request: CommitRequest<'_>) -> CommitResult<CommitOutcome> {
    let CommitRequest {
        store,
        index,
        keys,
        policy,
        batch,
    } = request;

    for attempt in 1..=policy.attempts {
        let caught = catch_up::catch_up(store, index, keys, &policy.retry).await?;

        candidate::check(index, &batch).await?;

        let committed = index.checkpoint().await?;
        let commitment = keyring::replicate(
            store,
            index,
            keys,
            &policy,
            &caught.listing,
            committed.as_ref(),
            &batch,
        )
        .await?;

        let Attempted::Committed(landed) =
            journal::commit(store, keys, &policy, &caught, commitment, &batch).await?
        else {
            debug!(
                attempt,
                "the commit slot was taken; catching up and retrying"
            );
            continue;
        };

        index
            .refresh(CommittedBatch {
                record: landed.record.clone(),
                materialized: batch.materialized.clone(),
            })
            .await?;

        let untrashed =
            settle::trash_removals(store, &policy, &caught.listing, &landed.record.removals).await;

        let checkpoint = settle::write_checkpoint(
            store,
            index,
            keys,
            &policy,
            &landed.record,
            &landed.snapshot_slot,
            caught.newest_checkpoint,
        )
        .await;

        return Ok(CommitOutcome {
            record: landed.record,
            attempts: attempt,
            checkpoint,
            untrashed,
        });
    }

    Err(CommitError::ConflictLimitReached {
        attempts: policy.attempts,
    })
}
