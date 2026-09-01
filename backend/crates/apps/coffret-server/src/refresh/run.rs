use std::time::Instant;

use coffret_device::CatchUpOutcome;
use tracing::info;

use crate::api_error::ApiError;
use crate::state::ServerState;

/// One catch-up, with nobody else replaying at the same time.
///
/// Both callers reach the Library through here, so the turn is taken and the
/// line is written once however the run was asked for; `operation` is which of
/// the two asked.
pub(super) async fn catch_up(
    state: &ServerState,
    operation: &'static str,
) -> Result<CatchUpOutcome, ApiError> {
    let started = Instant::now();
    let _turn = state.refreshes.turn().await;

    let outcome = state.library.catch_up().await?;
    // What it came to, and how long the caller waited — the wait for whoever was
    // replaying first included, since that is the time the request took. The
    // generations the catalog moved between are in the flow's own line and not
    // repeated here; nothing that arrived is named at all, an Entry Path being
    // the user's own name for their file (spec: EP-1).
    info!(
        operation,
        advanced = outcome.advanced(),
        gained = outcome.gained(),
        entries = outcome.entries_after,
        elapsed_ms = started.elapsed().as_millis(),
        "the catalog was caught up with the Library",
    );
    Ok(outcome)
}
