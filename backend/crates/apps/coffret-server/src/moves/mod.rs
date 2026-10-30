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
//! rebuilt. So it is not background work: the request waits for it, and is
//! answered with what it did.
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
//! the commit. What a move ended as is logged with the rest of the server's
//! finished work (spec: LA-12).

mod move_run;
pub use move_run::MoveRun;

mod move_status;
pub use move_status::MoveStatus;

// The refusals a preview can show, and that stop a move before it commits.
mod refusals;
pub use refusals::refusals_of;

// One move, from the pairs to the record and the relocations after it.
mod run;
pub use run::move_pairs;
