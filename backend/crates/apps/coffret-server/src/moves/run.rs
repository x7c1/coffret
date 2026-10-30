use std::time::Instant;

use coffret_device::{Error, Findings, RenameError, RenamePair, UNWATCHED};
use tracing::info;

use crate::activity_log::Ended;
use crate::api_error::ApiError;
use crate::finding::Finding;
use crate::pending_rows::TurnTaker;
use crate::reported::Reported;
use crate::state::ServerState;

use super::{refusals_of, MoveRun, MoveStatus};

/// Moves what `pairs` names, once, and answers what it did (spec: FM-15,
/// EP-10).
///
/// Under this device's turn at its pending rows, so nothing of this server's
/// own runs beside it (spec: LA-12, LA-31). Then the same refusals the preview shows,
/// asked again against the Library and the disk as they stand now, and then
/// the rename, whose record names every Entry the set renames, followed by this
/// device's relocations.
///
/// A move refused before its commit did nothing, and is answered
/// `409 refused_move` rather than logged (spec: LA-31): one the preview's
/// refusals stop, and one the rename's own plan refuses over the catalog its
/// catch-up brought to the head. Every other move is logged as finished work
/// (spec: LA-12): one that stopped short of the commit for any other reason
/// committed nothing, and is answered with what stopped it; one whose record
/// committed is done, and is answered with what it did — where this device's
/// files then could not be relocated, with what kept them, and the next sync
/// or fetch relocates them (spec: EP-10).
pub async fn move_pairs(state: &ServerState, pairs: Vec<RenamePair>) -> Result<MoveRun, ApiError> {
    let _turn = state.pending_rows.take(TurnTaker::Move).await;
    let library = state.unlocked()?;
    let preview = library.preview_move(&pairs).await?;
    if let Some(refusal) = refusals_of(&preview).into_iter().next() {
        return Err(refusal);
    }

    let started = Instant::now();
    let mut run = MoveRun::starting(pairs.clone());
    let result = library.rename_pairs(pairs, &UNWATCHED).await;
    let answer = match result {
        Ok(outcome) => {
            run.done(&outcome);
            Ok(())
        }
        // The plan, over the catalog the rename's catch-up brought to the
        // head, refused what the preview admitted: nothing was written, so
        // nothing is logged (spec: LA-31).
        Err(Error::Rename { cause }) if matches!(*cause, RenameError::Refused(_)) => {
            return Err(ApiError::from(Error::Rename { cause }));
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
        Err(error) => {
            // The Keyring replicas a commit put back before it failed stand on
            // Storage whatever became of the record, and a repair performed is
            // never silent (spec: KL-15).
            run.findings = Finding::all_of(&Findings::repaired_before(&error));
            let refusal = ApiError::from(error);
            run.status = MoveStatus::Stopped(Reported::of(&refusal));
            Err(refusal)
        }
    };

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
    state.activity.record(Ended::Move(run.clone()));
    answer.map(|()| run)
}
