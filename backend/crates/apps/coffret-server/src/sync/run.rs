use std::time::Instant;

use coffret_device::Findings;
use tracing::info;

use crate::adds::{log_undone, undo};
use crate::api_error::ApiError;
use crate::finding::Finding;
use crate::pending_rows::TurnTaker;
use crate::reported::Reported;
use crate::state::ServerState;
use crate::watched::Watched;

use super::{SyncRun, SyncStatus};

/// Carries the device's mapped folders into the Library, once.
///
/// [`sync`](coffret_device::OpenLibrary::sync) and nothing around it. Which
/// folders are walked is not an argument and cannot be — that is the device's
/// mappings (spec: EP-9) — and neither is what a change means: a file that is
/// new becomes an Entry, a file that changed replaces one where its Entry lives
/// in a Container of its own (spec: PK-12, PK-15), and everything else is
/// reported (spec: PK-14).
///
/// The run is this function's own value, published once at the end — and
/// while it runs, what the flow says of itself is written onto the run on
/// record. Unlike a fill this server counts nothing: a sync answers with what it
/// did when it has done it, and a count of its own moving in the middle of a
/// walk would be progress the flow never reported. What moves is the flow's own
/// step, through the same port the command line draws its line from.
///
/// # Stopped by a person
///
/// The run carries the adds armed for it (spec: LA-34), and one of them being
/// stopped asks the run to stop. The flow is raced against that ask, and the
/// ask wins only while the run has not begun committing: a run that has said
/// it is committing is never cut short, so a stop that came too late is
/// answered by what the run came to (spec: LA-35). A flow cut short before its
/// commit is a run interrupted, which the next one settles (spec: OC-2, OC-3):
/// what it sent to Storage is left to the cleanup of orphaned objects.
///
/// Then, still holding the turn and before this flow scans again, the run
/// undoes the adds a person stopped (spec: LA-36), logs each, and arms again
/// what it carried that nobody stopped. A run asked to stop while it waited
/// for its turn takes it all the same, because the undo is done under it.
pub(super) async fn sync(state: &ServerState) {
    let started = Instant::now();
    let mut run = SyncRun::starting();
    (run.adds, run.requested) = state.syncs.carried();
    // A run carrying an add stopped while it waited to be taken up is asked
    // from the start.
    if !state.adds.stopping_of(&run.adds).is_empty() {
        state.syncs.ask_stop();
    }

    // This device's pending rows are the sync's for the whole run, and a
    // freeze or a deletion already running holds them: wait for the turn
    // rather than being refused them (spec: OC-2). Before the keys, so the
    // wait is not counted as somebody being here (spec: DK-4) and a lock that
    // lands meanwhile stops this run below.
    let _turn = state.pending_rows.take(TurnTaker::Sync).await;

    // The keys, once, for the whole run, as the fill takes them: a lock that
    // lands mid-walk leaves this holding what it took and the run finishes,
    // and a run armed after one stops here rather than half carrying a folder
    // in (spec: DK-2).
    let library = match state.unlocked() {
        Ok(library) => library,
        Err(refusal) => {
            run.status = SyncStatus::Stopped(Reported::recorded(&refusal, "sync"));
            return finish(state, run, started);
        }
    };

    if !state.syncs.asked() {
        // No terminal to draw a line on, so the steps go where this process
        // says what it is doing: the work answer a browser polls (spec: LA-1).
        // It is the same port and the same steps the command line renders, so
        // the two shells cannot disagree about how far a run has got.
        let watched = Watched::by(|step| state.syncs.step(step));
        let ran = tokio::select! {
            // The ask first, so that one standing when both are ready is taken
            // — it can only be standing while the run has not said it is
            // committing, since saying so is what turns an ask away.
            biased;
            () = state.syncs.until_asked() => None,
            ran = library.sync(&watched) => Some(ran),
        };
        if let Some(ran) = ran {
            match ran {
                Ok(outcome) => {
                    // Counts and findings, and not `outcome.mappings`: a device
                    // that maps nothing reads here as a run with nothing new,
                    // and the explorer tells the two apart out of its own
                    // listing, which says whether a folder is mapped. So the
                    // work answer carries no count of mappings.
                    run.added = outcome.added.len();
                    run.findings = Finding::all_of(&Findings::from(&outcome));
                    run.status = SyncStatus::Done;
                }
                Err(error) => {
                    // What the run did before it stopped still happened: the
                    // Keyring replicas its commit put back stand on Storage
                    // whatever became of the batch, and a repair performed is
                    // never silent (spec: KL-15).
                    run.findings = Finding::all_of(&Findings::repaired_before(&error));
                    run.status =
                        SyncStatus::Stopped(Reported::recorded(&ApiError::from(error), "sync"));
                }
            }
            return finish(state, run, started);
        }
    }

    // Stopped by a person, before the commit: what the stopped adds wrote is
    // taken away under this turn, and what nobody stopped goes to the next run.
    let stopped = state.adds.stopping_of(&run.adds);
    for id in &stopped {
        if let Some(add) = state.adds.get(id) {
            log_undone(&state.activity, undo(&library, add).await);
            state.adds.end(id);
        }
    }
    let rest = run.adds.difference(&stopped).cloned().collect();
    let requested = run.requested;
    run.status = SyncStatus::Cancelled;
    finish(state, run, started);
    state.syncs.rearm(rest, requested);
}

/// Publishes what the sync came to, and records it.
///
/// Counts, a duration and an outcome. Nothing that was walked is named: a
/// local path never reaches a diagnostic event, and an Entry Path is the
/// user's own name for their file (spec: EL-1) — how many there were is
/// enough to read a run's account of itself.
fn finish(state: &ServerState, run: SyncRun, started: Instant) {
    info!(
        operation = "sync",
        outcome = run.status.as_str(),
        added = run.added,
        findings = run.findings.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "the mapped folders were carried into the Library",
    );
    // The adds a run that ended carried end with it, whatever it came to: a
    // stop that met its commit is answered by that commit (spec: LA-35).
    if run.status != SyncStatus::Cancelled {
        state.adds.end_all(&run.adds);
    }
    state.syncs.publish(&run);
}
