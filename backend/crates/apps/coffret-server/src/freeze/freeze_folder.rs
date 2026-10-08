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
