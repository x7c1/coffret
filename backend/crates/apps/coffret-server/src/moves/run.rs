use std::time::Instant;

use coffret_device::{Error, Findings, RenameError, UNWATCHED};
use tracing::info;

use crate::api_error::ApiError;
use crate::finding::Finding;
use crate::pending_rows::TurnTaker;
use crate::reported::Reported;
use crate::state::ServerState;

use super::{refusals_of, MoveRun, MoveStatus, PendingMove};

/// Carries out one accepted move, once, and logs what it came to
/// (spec: FM-15, EP-10, LA-31).
///
/// Under this device's turn at its pending rows, so nothing of this server's
/// own runs beside it (spec: LA-12, LA-31), and with the keys taken after that
/// wait rather than before it, so the wait is not counted as somebody being
/// here (spec: DK-4). Then the same refusals the preview shows, asked again
/// against the Library and the disk as they stand now, and then the rename,
/// whose record names every Entry the set renames, followed by this device's
/// relocations.
///
/// Every move ends in the log (spec: LA-12). One the preview's refusals stop,
/// one the rename's own plan refuses over the catalog its catch-up brought to
/// the head, and one stopped short of the commit for anything else — a locked
/// Library, Storage — committed nothing, and is logged as stopped with what
/// stopped it. One whose record committed is done, with what it did — where
/// this device's files then could not be relocated, with what kept them, and
/// the next sync or fetch relocates them (spec: EP-10).
pub(super) async fn move_pending(state: &ServerState, pending: PendingMove) {
    let started = Instant::now();
    let mut run = MoveRun::starting(pending.run, pending.pairs);

    let _turn = state.pending_rows.take(TurnTaker::Move).await;
    if let Err(refusal) = carry_out(state, &mut run).await {
        run.status = MoveStatus::Stopped(Reported::recorded(&refusal, "move"));
    }

    // Counts, a duration and an outcome, and no Entry Path: the paths are the
    // user's own names for their files (spec: EL-1).
    info!(
        operation = "move",
        outcome = run.status.as_str(),
        pairs = run.pairs.len(),
        entries = run.entries,
        relocated = run.relocated,
        left = run.left,
        unrelocated = run.unrelocated.is_some(),
        findings = run.findings.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "a move in the Library ended",
    );
    state.moves.finish(&run);
}

/// The move itself, under its turn: `Ok` once its record committed, whatever
/// became of this device's files, with `run` saying what it did; the refusal
/// that stopped it short of the commit otherwise.
async fn carry_out(state: &ServerState, run: &mut MoveRun) -> Result<(), ApiError> {
    let library = state.unlocked()?;
    let preview = library.preview_move(&run.pairs).await?;
    if let Some(refusal) = refusals_of(&preview).into_iter().next() {
        return Err(refusal);
    }
    match library.rename_pairs(run.pairs.clone(), &UNWATCHED).await {
        Ok(outcome) => {
            run.done(&outcome);
            Ok(())
        }
        // The record committed, and the catalog could not say which of this
        // device's files wait to follow it. Not a stopped move: the Library
        // holds the renames. The count is the preview's, made under this same
        // turn, since the failure does not carry the plan's.
        Err(Error::Rename { cause }) if matches!(*cause, RenameError::NotRelocated { .. }) => {
            let refusal = ApiError::from(Error::Rename { cause });
            run.unrelocated(
                preview.rename.renamed.len(),
                Reported::recorded(&refusal, "move"),
            );
            Ok(())
        }
        // The plan, over the catalog the rename's catch-up brought to the head,
        // refused what the preview admitted — nothing was written — or the
        // move stopped short of its commit for anything else. The Keyring
        // replicas a commit put back before it failed stand on Storage
        // whatever became of the record, and a repair performed is never
        // silent (spec: KL-15).
        Err(error) => {
            run.findings = Finding::all_of(&Findings::repaired_before(&error));
            Err(ApiError::from(error))
        }
    }
}
