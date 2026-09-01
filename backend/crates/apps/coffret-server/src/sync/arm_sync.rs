use std::sync::Arc;

use crate::state::ServerState;

use super::worker;

/// Asks for a sync, starting the work if nothing is running.
///
/// Returns at once: what it arms is a background task, and the caller is a
/// request with an answer of its own to give — which files it took, and which it
/// refused.
pub fn arm_sync(state: Arc<ServerState>) {
    if state.syncs.arm() {
        tokio::spawn(worker::work(state));
    }
}
