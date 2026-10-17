//! What a batch prepared against one head becomes on a newer one whose records
//! rename what it names (spec: CP-4, CP-7, EP-7).
//!
//! Each case but one prepares a batch against the head `ours` read, lets
//! `theirs` commit a rival record first, and then commits the batch — whose
//! catch-up replays the rival and whose rebase carries the batch over it.
//! The rival renames are batches built here rather than runs of the rename
//! operation, so that each case states exactly the record it races. The one
//! without a rival commits a rename the head itself does not admit, which is
//! refused with nothing for a rebase to carry it over. They
//! are this crate's own tests rather than suite cases because some of them
//! count what the in-memory store was asked to write, and every one of them is
//! about the commit flow rather than about a backend.

mod batches;
mod devices;

mod additions;
mod folder_prefix;
mod packs;
mod removals;
mod renames;
