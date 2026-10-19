use std::sync::Arc;

use crate::state::ServerState;

use super::{worker, Book};

/// Asks for `book` to be packed, starting the work if nothing is running.
///
/// Returns at once: what it arms is a worker, and the caller is a
/// request with an answer of its own to give — which pages it took, and which it
/// refused.
pub fn freeze_folder(state: Arc<ServerState>, book: Book) {
    if state.freezes.arm(book) {
        tokio::spawn(worker::work(state));
    }
}

/// Asks for the book a drop has just finished writing to be packed, as a run
/// of its own rather than one taken for the run under way (spec: PK-17).
///
/// Armed only once the drop has stopped receiving and its reservation is no
/// longer live, so the run it makes is one that decides the reservation:
/// every page the drop landed is in it by then. It does not join a run already
/// under way, even one of the drop's whole folder: that run read the
/// reservations while the drop was still receiving, so it spares this one, and
/// its walk may have gone past pages that landed after it. A book of the same
/// folder still waiting its turn is joined, since nothing has walked the folder
/// for it yet.
pub fn freeze_dropped(state: Arc<ServerState>, book: Book) {
    if state.freezes.arm_dropped(book) {
        tokio::spawn(worker::work(state));
    }
}
