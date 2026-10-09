//! Taking files and folders out of the Library, once a person has been shown
//! what that costs.
//!
//! The explorer offers "Delete…" on a file and on a folder: it asks
//! `GET /api/delete` what a deletion would take out and what it would rebuild,
//! shows that, and only a yes arms `POST /api/delete`.
//!
//! # One at a time, on a worker of its own
//!
//! A deletion that rebuilds a Pack reads the whole Pack back and uploads its
//! replacement (spec: PK-10), which can take as long as a freeze — so it is not
//! done while the request waits. It runs on a fourth worker beside the fill's,
//! the sync's and the freeze's, one deletion at a time: a second one armed while
//! the first is running waits its turn rather than racing it into the commit,
//! and one naming exactly what is already running or waiting is not queued
//! twice. What it has got to and what it came to are on the work answer, as the
//! other three are.
//!
//! It does not run beside a sync or a freeze, though: all three own this
//! device's pending rows for the whole of a run (spec: OC-2), so a deletion
//! waits for the one running before it starts rather than being refused the
//! rows.
//!
//! # What it does not touch
//!
//! Local files. A deletion changes the Library and nothing in a mapped folder,
//! so the person is told only what leaves the Library. What becomes of a file
//! whose Entry left is the next sync's to decide, on this device and every other
//! that holds one: an unedited copy goes to the desktop's trash and an edited
//! one is kept and reported (spec: EP-15).

mod delete_run;
pub use delete_run::{DeleteRun, RefusedPack};

mod delete_status;
pub use delete_status::DeleteStatus;

mod deletes;
pub use deletes::{DeleteReport, Deletes};

mod target;
pub use target::Target;

// One deletion, from the selection to the batch that commits it.
mod run;

// The worker itself, and what it puts back however it ends.
mod worker;

use std::sync::Arc;

use crate::state::ServerState;

/// Asks for `target` to be deleted, starting the work if nothing is running.
///
/// Returns at once: what it arms is a worker, and the caller is a request with
/// an answer of its own to give.
pub fn delete_target(state: Arc<ServerState>, target: Target) {
    if state.deletes.arm(target) {
        tokio::spawn(worker::work(state));
    }
}
