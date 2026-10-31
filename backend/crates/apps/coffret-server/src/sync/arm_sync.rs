use std::sync::Arc;

use crate::adds::AddId;
use crate::state::ServerState;

use super::worker;

/// Asks for a sync, starting the work if nothing is running.
///
/// Returns at once: what it arms is a worker, and the caller is a
/// request with an answer of its own to give — which files it took, and which it
/// refused.
///
/// `add` is the add the sync is to carry in, where a drop asks for it, and
/// `None` where a request asks for a sync of its own (spec: LA-34).
pub fn arm_sync(state: Arc<ServerState>, add: Option<AddId>) {
    if state.syncs.arm(add) {
        tokio::spawn(worker::work(state));
    }
}
