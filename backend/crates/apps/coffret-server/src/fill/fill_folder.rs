use std::sync::Arc;

use crate::folder::Folder;
use crate::state::ServerState;

use super::worker;

/// Makes `folder` the folder being filled, starting the work if nothing is
/// running.
///
/// Returns at once: what it arms is a background task, and the caller is a
/// request that has an Entry's bytes to answer with.
pub fn fill_folder(state: Arc<ServerState>, folder: Folder) {
    if state.fills.arm(folder) {
        tokio::spawn(worker::work(state));
    }
}
