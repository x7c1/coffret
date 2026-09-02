use std::sync::Arc;

use crate::folder::Folder;
use crate::state::ServerState;

use super::worker;

/// Asks for `folder` to be packed, starting the work if nothing is running.
///
/// Returns at once: what it arms is a background task, and the caller is a
/// request with an answer of its own to give — which pages it took, and which it
/// refused.
pub fn freeze_folder(state: Arc<ServerState>, folder: Folder) {
    if state.freezes.arm(folder) {
        tokio::spawn(worker::work(state));
    }
}
