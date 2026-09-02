//! Carrying what somebody just added into the Library, without them asking.
//!
//! Dropping a file into the explorer means one thing — add this — and adding is
//! not finished when the bytes reach a mapped folder. A file sitting in a folder
//! and no Entry is exactly the state the command line's `sync` exists to end, so
//! the server runs the same flow the person would have typed, at the moment
//! there is something to run it for.
//!
//! # Implicit, and one at a time
//!
//! There is no sync button and this is not one. What arms a sync is an upload
//! that landed at least one file. `POST /api/sync` exists for what the implicit
//! trigger cannot reach: taking the run up again after Storage stopped it, with
//! the files already sitting in the folder and nothing left to drop.
//!
//! One sync runs at a time, on one background task the server owns — a second
//! beside the fill's, because the two are different work over one Library and
//! neither waits on the other. A drop that arrives while a sync is running queues
//! exactly one follow-up run, and a second drop queues no more than that: what
//! the next run does is walk the mapped folders, and one walk finds both files.
//! There is nothing to lose by collapsing them and a whole scan to spend by not.
//!
//! That is where this differs from the fill, and the difference is the shape of
//! the work rather than a choice: a fill is *of a folder*, so arming another one
//! replaces it, and a sync is of the device's mappings entire, so arming one
//! twice asks for the same thing twice.
//!
//! # What it is not allowed to do
//!
//! It runs [`sync`](coffret_device::OpenLibrary::sync) and nothing else — no
//! narrowing, no selection, no second reading of what a sync means. A run that
//! returns `Ok` has still to be read for what it left alone (spec: PK-14,
//! EP-12), and those findings reach the browser as [`Noted`] rather than being
//! swallowed: somebody who dropped a file is owed the answer that it was not
//! backed up, and they are not at a terminal to be told there.

mod arm_sync;
pub use arm_sync::arm_sync;

mod sync_activity;
pub use sync_activity::SyncActivity;

mod sync_status;
pub use sync_status::SyncStatus;

mod syncs;
pub use syncs::Syncs;

// Everything the server knows about syncing, in the one value the others read
// and write it through.
mod progress;

// One run of the flow, and what it found.
mod run;

// The background task itself, and what it puts back however it ends.
mod worker;
