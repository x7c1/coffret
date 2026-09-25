use std::time::Instant;

use coffret_device::Findings;
use tracing::info;

use crate::api_error::ApiError;
use crate::finding::Finding;
use crate::reported::Reported;
use crate::state::ServerState;
use crate::watched::Watched;

use super::{SyncActivity, SyncStatus};

/// Carries the device's mapped folders into the Library, once.
///
/// [`sync`](coffret_device::OpenLibrary::sync) and nothing around it. Which
/// folders are walked is not an argument and cannot be — that is the device's
/// mappings (spec: EP-9) — and neither is what a change means: a file that is
/// new becomes an Entry, a file that changed replaces one where its Entry lives
/// in a Container of its own (spec: PK-12, PK-15), and everything else is
/// reported (spec: PK-14).
///
/// The activity is this function's own value, published once at the end — and
/// while it runs, what the flow says of itself is written onto the activity on
/// record. Unlike a fill this server counts nothing: a sync answers with what it
/// did when it has done it, and a count of its own moving in the middle of a
/// walk would be progress the flow never reported. What moves is the flow's own
/// step, through the same port the command line draws its line from.
pub(super) async fn sync(state: &ServerState) {
    let started = Instant::now();
    let mut activity = SyncActivity::starting();

    // The keys, once, for the whole run, as the fill takes them: a lock that
    // lands mid-walk leaves this holding what it took and the run finishes,
    // and a run armed after one stops here rather than half carrying a folder
    // in (spec: DK-2).
    let library = match state.unlocked() {
        Ok(library) => library,
        Err(refusal) => {
            activity.status = SyncStatus::Stopped;
            activity.stopped = Some(Reported::recorded(&refusal, "sync"));
            return finish(state, activity, started);
        }
    };

    // No terminal to draw a line on, so the steps go where this process says
    // what it is doing: the activity a browser polls (spec: LA-1). It is the
    // same port and the same steps the command line renders, so the two shells
    // cannot disagree about how far a run has got.
    let watched = Watched::by(|step| state.syncs.step(step));
    match library.sync(&watched).await {
        Ok(outcome) => {
            activity.added = outcome.added.len();
            activity.findings = Findings::from(&outcome)
                .iter()
                .filter_map(Finding::of)
                .collect();
            activity.status = SyncStatus::Done;
        }
        Err(error) => {
            activity.status = SyncStatus::Stopped;
            activity.stopped = Some(Reported::recorded(&ApiError::from(error), "sync"));
        }
    }
    finish(state, activity, started);
}

/// Publishes what the sync came to, and records it.
///
/// Counts, a duration and an outcome. Nothing that was walked is named: a
/// local path never reaches a diagnostic event, and an Entry Path is the
/// user's own name for their file (spec: EL-1) — how many there were is
/// enough to read a run's account of itself.
fn finish(state: &ServerState, activity: SyncActivity, started: Instant) {
    info!(
        operation = "sync",
        outcome = activity.status.as_str(),
        added = activity.added,
        findings = activity.findings.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "the mapped folders were carried into the Library",
    );
    state.syncs.publish(&activity);
}
