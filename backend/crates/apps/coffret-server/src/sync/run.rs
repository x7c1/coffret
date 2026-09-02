use std::time::Instant;

use coffret_device::Findings;
use tracing::info;

use crate::api_error::ApiError;
use crate::noted::Noted;
use crate::reported::Reported;
use crate::state::ServerState;

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
/// The activity is this function's own value, published once at the end. Unlike
/// a fill there is nothing to publish along the way: a sync answers with what it
/// did when it has done it, and a count that moved in the middle of a walk would
/// be this server inventing progress the flow does not report.
pub(super) async fn sync(state: &ServerState) {
    let started = Instant::now();
    let mut activity = SyncActivity::starting();

    match state.library.sync().await {
        Ok(outcome) => {
            activity.added = outcome.added.len();
            activity.noted = Findings::from(&outcome)
                .iter()
                .filter_map(Noted::of)
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
/// Counts, a duration and an outcome. Nothing that was walked is named: a local
/// path never reaches a log line, and an Entry Path is the user's own name for
/// their file (spec: EP-1) — how many there were is enough to read a run's
/// account of itself.
fn finish(state: &ServerState, activity: SyncActivity, started: Instant) {
    info!(
        operation = "sync",
        outcome = activity.status.as_str(),
        added = activity.added,
        noted = activity.noted.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "the mapped folders were carried into the Library",
    );
    state.syncs.publish(&activity);
}
