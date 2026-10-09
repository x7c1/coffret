use std::sync::Arc;

use coffret_device::{DegradedKeyring, UnheldParcel};

use crate::folder::Folder;
use crate::state::ServerState;

use super::worker;

/// Follows a fetch into `folder`, starting the work if nothing is running.
///
/// Latest wins: a fetch that landed somewhere else is a person who has moved on,
/// and the fill goes with them. A folder somebody asked for by name goes through
/// [`queue_folder`] instead.
///
/// `heard` is what the fetch that armed it found of the committed Keyring,
/// where it had to step over a position of the set (spec: KL-5, KL-15), and
/// `unheld` the kept parcels it found not held and read again (spec: PK-21);
/// the run that takes this arming up reports both as its own (see
/// `Progress::heard` and `Progress::unheld`).
///
/// Returns at once: what it arms is a worker, and the caller is a
/// request that has an Entry's bytes to answer with.
pub fn fill_folder(
    state: Arc<ServerState>,
    folder: Folder,
    heard: Option<DegradedKeyring>,
    unheld: Vec<UnheldParcel>,
) {
    if state.fills.arm(folder, heard, unheld) {
        tokio::spawn(worker::work(state));
    }
}

/// Takes `folder` up because somebody asked for it by name, starting the work if
/// nothing is running.
///
/// It waits its turn rather than superseding what is running: a button pressed on
/// purpose is a decision about that folder, and a second press must bring that
/// folder over too rather than erasing the first. Returns at once, for the
/// reason [`fill_folder`] does.
pub fn queue_folder(state: Arc<ServerState>, folder: Folder) {
    if state.fills.queue(folder) {
        tokio::spawn(worker::work(state));
    }
}
