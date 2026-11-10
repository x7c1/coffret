//! Moving and renaming files and folders in the Library, once a person has
//! been shown what that does to this device's files.
//!
//! The explorer offers moving a selection by dragging it onto a folder, and
//! "Rename…" and "Move to…" in a row's menu. Each is a set of `(from, to)`
//! pairs: it asks `GET /api/move` what the set would do, shows that, and only
//! a yes asks `POST /api/move`.
//!
//! # One record, and nothing uploaded
//!
//! A move is a rename of Entries (spec: FM-15): one Journal record names every
//! Entry the set renames, nothing is read, encrypted or uploaded, and no Pack is
//! rebuilt.
//!
//! # Background work, on a worker of its own
//!
//! It is short work, but it is not done while the request waits (spec: LA-31):
//! it takes this device's turn at its pending rows, behind a sync or a freeze
//! that may hold it for as long as a book takes to pack, and it catches up to
//! the Library's head first (spec: CK-9), which is a round trip to Storage. So
//! the `POST` accepts it and answers at once, and a fifth worker beside the
//! fill's, the sync's, the freeze's and the deletion's carries the moves out
//! one at a time, in the order they were accepted. The work answer lists every
//! move accepted and not yet ended, and the log holds every one that ended
//! (spec: LA-12).
//!
//! A move accepted is shown at its new place before it has committed. So a
//! second move, a deletion or a drop that touches the rows of a pending move
//! is refused up front rather than queued behind it: it would act on rows
//! that are not yet where the person was shown them.
//!
//! # This device's files
//!
//! Once the record has committed, this device's own files of the renamed
//! Entries follow them at once (spec: EP-10): with one filesystem rename
//! within a mapped folder or across two on one filesystem, and otherwise they
//! leave this device — an unedited one is discarded into the desktop's trash,
//! and the Entry is in the Library and not on this device at its new path.
//! That is why a set is refused before anything is written where any file of
//! this device's in it is edited, or anything under it is still on its way into
//! the Library: leaving the device would lose what the Library does not hold.
//!
//! # Taking turns
//!
//! A move takes its turn at this device's pending rows as a sync, a freeze
//! and a deletion do (spec: OC-2), so its relocations never run beside a
//! sync's scan of the same folders, and two moves never race each other into
//! the commit.

use std::sync::Arc;

use coffret_device::RenamePair;

use crate::api_error::ApiError;
use crate::state::ServerState;

mod move_run;
pub use move_run::MoveRun;

mod move_status;
pub use move_status::MoveStatus;

// The moves accepted and not yet ended.
mod queue;
pub use queue::{Moves, PendingMove};

// The refusals a preview can show, and that stop a move before it commits.
mod refusals;
pub use refusals::refusals_of;

// One move, from the pairs to the record and the relocations after it.
mod run;

// The worker itself, and what it puts back however it ends.
mod worker;

/// Accepts a move of `pairs`, starting the work if nothing is running, and
/// answers which move it is.
///
/// Returns at once: what it arms is a worker, and the caller is a request with
/// an answer of its own to give. Refused, with nothing accepted, where a
/// pending move touches any of the same rows.
pub fn move_pairs(state: Arc<ServerState>, pairs: Vec<RenamePair>) -> Result<u64, ApiError> {
    let (run, start) = state.moves.accept(pairs)?;
    if start {
        tokio::spawn(worker::work(state));
    }
    Ok(run)
}
