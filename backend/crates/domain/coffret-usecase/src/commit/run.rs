use tracing::debug;

use crate::commit::commit_error::{CommitError, CommitResult};
use crate::commit::commit_failure::CommitFailure;
use crate::commit::commit_outcome::CommitOutcome;
use crate::commit::commit_request::CommitRequest;
use crate::commit::committing::Committing;
use crate::commit::journal::Attempted;
use crate::commit::keyring_repair::KeyringRepair;
use crate::commit::{after_commit, candidate, catch_up, journal, keyring};
use crate::committed_batch::CommittedBatch;

/// Makes a prepared batch the Library's next committed state.
///
/// The whole flow, in the order the commit protocol fixes: catch the Index up
/// to the current head (spec: CK-9), refuse the batch if a Container it removes
/// is no longer current (spec: CP-18) or its Entry Paths would collide
/// (spec: EP-6), repair the committed Keyring if it has lost replicas
/// (spec: KL-11, KL-13), write and verify the Keyring generation the commit will
/// select (spec: CP-8, KL-2), and consume the head's commit slot with the Journal
/// record (spec: CP-2, CP-3). Creating that object is the commit point: before
/// it the batch has changed nothing, and after it the batch's additions and
/// removals are part of the current Container set, never partially (spec: CP-1).
///
/// The repair is where it is because KL-11 puts it there: a committed set that
/// has lost replicas must be complete again *before another write*, so it
/// happens on every attempt, after the catch-up that says which set is
/// committed and before anything of this batch's own reaches Storage. One that
/// cannot complete refuses the commit and commits nothing; the gate is never
/// partially relaxed, and the next run tries again (spec: KL-16). Every repair
/// a run performed is handed back for the caller to surface, one per attempt
/// that put a position back, because replica loss and its repair are never
/// silent (spec: KL-15): on [`CommitOutcome`] where the run committed, and on
/// [`CommitFailure`] where it ended in an error, whichever step raised it — the
/// replicas an attempt rewrote stand on Storage whatever became of the batch.
/// What the examination that refused the commit put back before it stopped is
/// the one exception, and travels on [`CommitError::UnrepairedKeyring`] with
/// the generation it is about.
///
/// Losing the slot is a normal outcome and not an error. The attempt rebases —
/// the same catch-up, the same removal and uniqueness checks, a fresh Keyring
/// generation over the new current set — and tries again, up to
/// [`CommitPolicy::attempts`](super::CommitPolicy::attempts). Nothing is ever
/// resolved by comparing timestamps (spec: CP-4, CP-7, CP-18, EP-7).
///
/// What happens after the record exists cannot un-commit it (spec: CP-1).
/// Trashing the removed Containers and writing the head's Snapshot are both
/// retryable later and neither failing fails the call: [`CommitOutcome`] reports
/// what of the two did not finish (spec: CK-8).
///
/// Refreshing the Index is the one post-commit step that does fail the call, and
/// it fails it with the batch committed. The caller that meets this error is
/// stale rather than uncommitted: offering the same batch again would refuse it,
/// once a later catch-up has replayed its record, as removing Containers that
/// record already removed or as an Entry Path collision with its own Containers
/// (spec: CP-18, EP-6). That is why it is an error and not a
/// finding on [`CommitOutcome`] — there is nothing the caller may do with this
/// batch next.
///
/// Neither half of the refresh is lost by failing here. The Library-wide half is
/// the record, which any later catch-up replays. The device-local half — which
/// files this device put on disk (spec: EP-10) and which spools stop being
/// pending (spec: OC-2) — survives in the pending rows this batch's spool step
/// wrote: a row whose Container a caught-up Index says is current is proof its
/// record landed, and completing the interrupted bookkeeping from it is what the
/// next sync run does before it scans (spec: OC-7, CP-1).
///
/// The request's progress hears how far each attempt has got (see
/// [`Phase::Committing`](crate::Phase::Committing)).
///
/// The Keyring replicas of an attempt that then lost the race stay on Storage as
/// an uncommitted candidate. That is what they are meant to be: a candidate set
/// selects nothing until a commit names its exact tuple (spec: KL-3), and
/// disposing of one is orphan cleanup's business (spec: KL-12, OC-2).
pub async fn commit_batch(request: CommitRequest<'_>) -> Result<CommitOutcome, CommitFailure> {
    // Kept here rather than read off the last attempt, because an attempt that
    // repaired the set and then lost the slot is the one that did the work: the
    // rebase examines the head the winner left and finds nothing of this run's
    // own repair to report (spec: CP-4, KL-15). And kept outside the attempts
    // themselves, so that a step failing after a repair cannot take the repair
    // down with it.
    let mut repairs: Vec<KeyringRepair> = Vec::new();
    match attempt_until_committed(request, &mut repairs).await {
        Ok(outcome) => Ok(CommitOutcome { repairs, ..outcome }),
        Err(error) => Err(CommitFailure {
            error: Box::new(error),
            repairs,
        }),
    }
}

/// The attempts themselves, each repair an attempt performed added to `repairs`
/// as soon as its examination hands it back.
///
/// The outcome it returns carries no repairs of its own: [`commit_batch`] puts
/// `repairs` on whichever of the outcome or the failure the run ended in.
async fn attempt_until_committed(
    request: CommitRequest<'_>,
    repairs: &mut Vec<KeyringRepair>,
) -> CommitResult<CommitOutcome> {
    let CommitRequest {
        store,
        index,
        keys,
        policy,
        batch,
        progress,
        degraded,
    } = request;
    let committing = Committing::under(&policy, progress);

    for attempt in 1..=policy.attempts {
        committing.begun();
        let caught = catch_up::catch_up(store, index, keys, &policy.retry).await?;

        candidate::check(index, &batch).await?;

        let committed = index.checkpoint().await?;
        let mut examined = match committed.as_ref() {
            Some(checkpoint) => {
                let examined =
                    keyring::examine(store, keys, &policy, &caught.listing, checkpoint.keyring())
                        .await;
                // The examination is the exhaustive walk of the same committed
                // generation a caller's earlier read walked, and it says what
                // it found and what it put back (spec: KL-11, KL-15). So a
                // finding travelling with the request is spoken for once the
                // examination has spoken — see [`spoke`] — and the caller's
                // guard goes quiet; otherwise the guard stays armed and its
                // drop speaks instead.
                if let Some(report) = degraded {
                    if spoke(&examined) {
                        report.examined();
                    }
                }
                examined?
            }
            // A Library with no committed head has no committed Keyring, so
            // there is nothing to examine and nothing to repair (spec: FM-13).
            None => keyring::Examined::first(),
        };
        repairs.extend(examined.take_repair());
        let commitment =
            keyring::replicate(store, index, keys, &policy, &examined, &batch, committing).await?;

        super::pending_provenance::mark_attempt(index, &batch).await?;

        let Attempted::Committed(landed) =
            journal::commit(store, keys, &policy, &caught, commitment, &batch).await?
        else {
            debug!(
                attempt,
                "the commit slot was taken; catching up and retrying"
            );
            continue;
        };
        committing.committed();

        index
            .refresh(CommittedBatch {
                record: landed.record.clone(),
                materialized: batch.materialized.clone(),
            })
            .await?;

        let untrashed =
            after_commit::trash_removals(store, &policy, &caught.listing, landed.record.removals())
                .await;

        let checkpoint = after_commit::write_checkpoint(
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
            repairs: Vec::new(),
        });
    }

    Err(CommitError::ConflictLimitReached {
        attempts: policy.attempts,
    })
}

/// Whether an examination has said its piece about the committed set, whatever
/// it came back with (spec: KL-15).
///
/// One that returns has — its repair travels on the outcome, and a set it found
/// complete is one there was nothing to say about. One that stopped because a
/// repair would not complete has written its own line before it refused. Every
/// other refusal, [`CommitError::KeyringUnreadable`] among them, left the walk
/// before any line was written, so the caller's finding is still unsaid.
fn spoke<T>(examined: &CommitResult<T>) -> bool {
    match examined {
        Ok(_) | Err(CommitError::UnrepairedKeyring { .. }) => true,
        Err(_) => false,
    }
}
