use std::time::Instant;

use coffret_device::Findings;
use tracing::info;

use crate::api_error::ApiError;
use crate::finding::Finding;
use crate::reported::Reported;
use crate::state::ServerState;
use crate::watched::Watched;

use super::{DeleteRun, DeleteStatus, Target};

/// Deletes what one target names, once.
///
/// [`delete`](coffret_device::OpenLibrary::delete) and nothing around it: what
/// is removed, what is rebuilt and what is refused is the use case's answer
/// over the catalog as the run finds it, which is why a preview and a run can
/// differ only where the Library moved in between (spec: PK-9).
///
/// The keys are taken once for the whole run, as every other flow here takes
/// them: a lock that lands while a Pack is being rebuilt leaves this holding
/// what it took, so the batch is committed or abandoned whole (spec: CP-1,
/// DK-2), and a deletion armed after one stops here.
///
/// It starts only once no sync and no freeze is running, and takes the keys
/// after that wait rather than before it, so the wait is not counted as
/// somebody being here (spec: DK-4). See [`until_pending_rows_are_free`].
pub(super) async fn delete(state: &ServerState, target: Target) {
    let started = Instant::now();
    let selection = target.selection();
    let mut run = DeleteRun::starting(target);

    until_pending_rows_are_free(state).await;
    let library = match state.unlocked() {
        Ok(library) => library,
        Err(refusal) => {
            run.status = DeleteStatus::Stopped(Reported::recorded(&refusal, "delete"));
            return finish(state, run, started);
        }
    };

    let watched = Watched::by(|step| state.deletes.step(step));
    match library.delete(selection, &watched).await {
        Ok(outcome) => run.done(&outcome),
        Err(error) => {
            // The Keyring replicas a commit put back before it failed stand on
            // Storage whatever became of the batch, and a repair performed is
            // never silent (spec: KL-15).
            run.findings = Finding::all_of(&Findings::repaired_before(&error));
            run.status =
                DeleteStatus::Stopped(Reported::recorded(&ApiError::from(error), "delete"));
        }
    }
    finish(state, run, started);
}

/// Waits until neither a freeze nor a sync is running or waiting to run.
///
/// A deletion, a freeze and a sync each own this device's pending rows for the
/// whole of their run (spec: OC-2), and a second owner is refused at once
/// rather than made to wait — so a deletion started beside a freeze packing a
/// book would stop as a server that could not answer, having committed nothing.
/// Waiting here puts a confirmed deletion after the work already running, which
/// is the order the person asked for them in.
///
/// Asked again after both waits, because a freeze can be armed while a sync is
/// being waited for. A sync armed in the moment between this returning and the
/// run taking the rows can still meet a deletion holding them; this narrows the
/// two to one at a time on this server and does not make them one queue.
async fn until_pending_rows_are_free(state: &ServerState) {
    loop {
        state.freezes.until_idle().await;
        state.syncs.until_idle().await;
        if !state.freezes.running() {
            return;
        }
    }
}

/// Publishes what the deletion came to, and records it.
///
/// Counts, a duration and an outcome, and no Entry Path: the paths are the
/// user's own names for their files (spec: EL-1).
fn finish(state: &ServerState, run: DeleteRun, started: Instant) {
    info!(
        operation = "delete",
        outcome = run.status.as_str(),
        folder = run.target.folder.is_some(),
        paths = run.target.paths.len(),
        entries = run.entries,
        removed = run.removed,
        rebuilt = run.rebuilt,
        refused = run.refused.len(),
        missing = run.missing.len(),
        findings = run.findings.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "a deletion from the Library ended",
    );
    state.deletes.publish(&run);
}
