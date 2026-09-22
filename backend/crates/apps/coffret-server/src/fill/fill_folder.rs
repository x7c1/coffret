use std::sync::Arc;

use crate::folder::Folder;
use crate::state::ServerState;

use super::worker;

/// Follows a fetch into `folder`, starting the work if nothing is running.
///
/// Latest wins: a fetch that landed somewhere else is a person who has moved on,
/// and the fill goes with them. A folder somebody asked for by name goes through
/// [`queue_folder`] instead.
///
/// Returns at once: what it arms is a background task, and the caller is a
/// request that has an Entry's bytes to answer with.
pub fn fill_folder(state: Arc<ServerState>, folder: Folder) {
    if state.fills.arm(folder) {
        tokio::spawn(worker::work(state));
    }
}

/// Takes `folder` up because somebody asked for it by name, starting the work if
/// nothing is running.
///
/// It waits its turn rather than displacing what is running: a button pressed on
/// purpose is a decision about that folder, and a second press must bring that
/// folder over too rather than erasing the first. Returns at once, for the
/// reason [`fill_folder`] does.
pub fn queue_folder(state: Arc<ServerState>, folder: Folder) {
    if state.fills.queue(folder) {
        tokio::spawn(worker::work(state));
    }
}
