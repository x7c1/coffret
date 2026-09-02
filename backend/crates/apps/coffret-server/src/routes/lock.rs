use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;
use tracing::info;

use crate::state::ServerState;

/// What a lock came to.
///
/// One field, and it is always `true`. That is not a placeholder: it is the
/// guarantee the route exists to make — the Library is locked, and it was locked
/// before this answer was written (spec: DK-3). A lock that had not taken effect
/// would not have answered.
///
/// Nothing else is in it. Whether this call was the one that did the locking, or
/// whether the server was already locked when it arrived, is the same state to
/// whoever asked and is left to the log.
#[derive(Serialize)]
pub struct LockedDto {
    locked: bool,
}

/// `POST /api/lock`
///
/// Locks the Library (spec: DK-3).
///
/// The explicit half of the two ways a served Library is locked, the other
/// being the idle interval. It empties the one cell the keys are held in, so
/// every request that arrives after this answer is refused with the Passphrase
/// asked for, and every piece of work that was already running finishes with the
/// handle it took (spec: DK-2).
///
/// It is inside the same admission fence as every other route, and has to be:
/// locking somebody's Library is a thing to be done to it, and a page on another
/// site aiming this at a loopback port would be shutting a Library it may not
/// even read.
///
/// It needs no key of its own, so it goes on answering once the server is
/// locked.
pub async fn lock(State(state): State<Arc<ServerState>>) -> Json<LockedDto> {
    if state.lock() {
        // Only the call that did it. A second one is the same state arrived at
        // again, and a line per press would make a log read as though the keys
        // had been wiped twice.
        info!(
            operation = "lock",
            how = "asked",
            "the Library was locked because somebody asked",
        );
    }
    Json(LockedDto { locked: true })
}
