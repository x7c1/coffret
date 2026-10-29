use coffret_model::{Generation, JournalRecord};
use tracing::debug;

use crate::commit::catch_up::CaughtUp;
use crate::commit::commit_error::{CommitError, CommitResult};
use crate::commit::commit_failure::CommitFailure;
use crate::commit::commit_outcome::CommitOutcome;
use crate::commit::commit_policy::CommitPolicy;
use crate::commit::commit_request::CommitRequest;
use crate::commit::committing::Committing;
use crate::commit::control_keys::ControlKeys;
use crate::commit::journal::Attempted;
use crate::commit::keyring_repair::KeyringRepair;
use crate::commit::prepared_batch::PreparedBatch;
use crate::commit::{after_commit, candidate, catch_up, first_head, journal, keyring, rebase};
use crate::committed_batch::CommittedBatch;
use crate::device_state::{LocalObservation, ReservationId};
use crate::index::Index;
use crate::index_error::IndexError;
use crate::object_store::ObjectStore;

/// Makes a prepared batch the Library's next committed state.
///
/// The whole flow, in the order the commit protocol fixes: catch the Index up
/// to the current head (spec: CK-9), refuse the batch if a Container it removes
/// is no longer current (spec: CP-18) or its Entry Paths would collide
/// (spec: EP-6), repair the committed Keyring if it has lost replicas
/// (spec: KL-11, KL-13), write and verify the Keyring generation the commit will
/// select (spec: CP-8, KL-2), and consume the head's commit slot with the Journal
/// record (spec: CP-2, CP-3) — having first recorded beside the batch's pending
/// rows that a create into that slot is about to be sent, which is what a later
/// settle reads to prove the batch did or did not commit there (spec: OC-2,
/// OC-3). Creating that object is the commit point: before
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
/// Every attempt, the first included, carries the batch over the records
/// committed after the head it was prepared against: their renames are applied
/// to its pending changes by `(Container ID, entry number)`, so an update lands
/// at the path a rename gave its Entry rather than bringing the old name back,
/// and a write to one Entry or Entry Path from two sides is refused as
/// [`CommitError::RebaseConflict`] (spec: EP-7).
///
/// Every commit starts from a head, because creating a Library writes its first
/// one ([`write_first_head`](super::write_first_head)). A Library created before
/// that has none until something commits, and its first run writes the empty
/// `head-0` before the batch and then commits the batch on top of it through
/// the slot that head carries; that is not an attempt at the batch, and it is
/// made at most once a run (spec: CP-2).
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
/// batch next. A refresh another process got to first — one refused as
/// [`IndexError::AlreadyApplied`] — is not such a failure: the record is in the
/// Index already, so the run records the rest of the batch through
/// [`Index::complete_applied`] and completes (spec: CK-9).
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
/// disposing of one is orphan cleanup's business (spec: KL-12, OC-2) — until a
/// later generation supersedes its number, when `prune` takes it with the
/// committed set beside it (spec: KL-18).
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
        short,
    } = request;
    let committing = Committing::under(&policy, progress);
    // Whether this run has written the Library's first head, or found it
    // created by another writer first; it tries at most once (see below).
    let mut wrote_first_head = false;
    // The head the batch was prepared against, and the records committed
    // since, read once each however many attempts the run takes
    // (spec: CP-4, EP-7).
    let base = match batch.base {
        Some(base) => Some(base),
        None => index
            .checkpoint()
            .await?
            .map(|checkpoint| checkpoint.head_generation()),
    };
    let mut intervening: Vec<JournalRecord> = Vec::new();

    let mut attempt = 0;
    while attempt < policy.attempts {
        committing.begun();
        let caught = catch_up::catch_up(store, index, keys, &policy.retry).await?;

        // A Library with no head has its empty first head written before the
        // batch, so the batch never reserves a slot of its own (spec: CP-2);
        // see [`commit_batch`]. An empty batch is itself such a head and goes
        // straight on.
        if caught.head.is_none() && !batch.is_empty() {
            if wrote_first_head {
                return Err(CommitError::FirstHeadUnlisted);
            }
            wrote_first_head = true;
            if let Attempted::Committed(landed) =
                first_head::write(store, keys, &policy, committing, &PreparedBatch::default())
                    .await?
            {
                refresh(index, &landed.record, &[], &[]).await?;
            }
            // Not an attempt at this batch: the next pass catches up onto the
            // head just written, or onto whichever writer's got there first.
            continue;
        }
        attempt += 1;

        // Shadowed for the rest of the attempt, so that nothing below commits
        // the batch as prepared rather than as it stands on this head; the
        // next attempt rebases from the prepared one again (spec: EP-7).
        read_intervening(store, keys, &policy, &caught, base, &mut intervening).await?;
        let batch = rebase::rebase(index, &batch, &intervening).await?;
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
                if let Some(report) = short {
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

        let Some(aim) = journal::aim(store, &policy, &caught, committed.as_ref()).await? else {
            debug!(
                attempt,
                "the catalog moved past the head this attempt caught up to; retrying"
            );
            continue;
        };

        super::pending_provenance::mark_attempt(index, &batch, &aim.attempted()).await?;

        let Attempted::Committed(landed) =
            journal::commit(store, keys, &policy, aim, commitment, &batch).await?
        else {
            debug!(
                attempt,
                "the commit slot was taken; catching up and retrying"
            );
            continue;
        };
        committing.committed();

        refresh(index, &landed.record, &batch.materialized, &batch.released).await?;

        let untrashed =
            after_commit::trash_removals(store, &policy, &caught.listing, landed.record.removals())
                .await;

        let checkpoint = after_commit::write_checkpoint(
            store,
            index,
            keys,
            &policy,
            &landed.record,
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

/// Refreshes the Index with this device's own batch, just committed.
///
/// Another process over the same Index — a server answering a browser while a
/// `sync` runs in a terminal — may replay the record between the create and
/// this refresh, and the refresh is then refused as
/// [`IndexError::AlreadyApplied`] with nothing of the batch's own recorded
/// (spec: CK-9). The record is in the catalog either way, and the record at
/// that generation is this batch's, so the refusal says only that the
/// device-local half is still to do: the Index records it as one operation and
/// the commit completes (spec: EP-10, OC-2, PK-17).
async fn refresh(
    index: &dyn Index,
    record: &JournalRecord,
    materialized: &[LocalObservation],
    released: &[ReservationId],
) -> CommitResult<()> {
    let batch = || CommittedBatch {
        record: record.clone(),
        materialized: materialized.to_vec(),
        released: released.to_vec(),
    };
    match index.refresh(batch()).await {
        Ok(()) => Ok(()),
        Err(IndexError::AlreadyApplied {
            generation,
            checkpoint,
        }) => {
            debug!(
                generation = generation.get(),
                checkpoint = checkpoint.get(),
                "another process applied this commit's record first; completing the rest",
            );
            Ok(index.complete_applied(batch()).await?)
        }
        Err(error) => Err(error.into()),
    }
}

/// Extends `held` with the Journal records after the last one it holds — or
/// after `base`, where it holds none — up to the head `caught` reached.
///
/// These are the records the batch did not see: committed after the head it
/// was prepared against (spec: CP-4, EP-7). An attempt that lost the slot
/// keeps what it read, so the next one reads only the records committed since.
async fn read_intervening(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    caught: &CaughtUp,
    base: Option<Generation>,
    held: &mut Vec<JournalRecord>,
) -> CommitResult<()> {
    let Some(head) = caught.head_generation() else {
        return Ok(());
    };
    let after = held.last().map(JournalRecord::generation).or(base);
    let from = match after {
        Some(after) => after.next()?.get(),
        // Prepared against a Library with no head: every record is one the
        // batch did not see (spec: FM-13).
        None => Generation::FIRST.get(),
    };
    let generations = (from..=head.get()).map(|number| {
        Generation::new(number).expect("a number no larger than the head is a generation")
    });
    held.extend(
        catch_up::read_records(store, keys, &policy.retry, &caught.listing, generations).await?,
    );
    Ok(())
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
