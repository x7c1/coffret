//! Packing a book somebody just brought in, without them asking.
//!
//! A scanned book is one folder of a few hundred page images, and carrying it in
//! the way a dropped photograph is carried in would make it a few hundred
//! Storage Objects — a few hundred uploads, a few hundred provider calls to open
//! it again, and a few hundred more for every rebuild after that. `freeze` is
//! the flow that puts those pages into Packs instead (spec: PK-1, PK-7), and
//! until now the only way to reach it was the command line.
//!
//! # Implicit, and one book at a time
//!
//! There is no "pack this" button and this is not one. What arms a freeze is a
//! drop onto a folder the person made in the browser a moment ago: a folder with
//! nothing in it that they are filling in one gesture is a book being imported,
//! and they have already said everything there is to say about it.
//! `POST /api/freeze` exists for what that trigger cannot reach — a freeze
//! Storage stopped, with the pages already sitting in the folder and nothing
//! left to drop — exactly as `POST /api/fill` and `POST /api/sync` do.
//!
//! Dropping into a folder the Library already has is not this. That stays the
//! sync's: the files are going in beside Entries that are already committed, and
//! a freeze there would pack the new pages into Packs of its own beside the ones
//! the folder already holds — existing Packs are never a freeze's inputs and it
//! never rewrites them (spec: PK-1, PK-2), and regrouping across invocations is
//! repack's and compaction's work (spec: PK-8). Which is a call for whoever is
//! at the command line to make, rather than one to read out of a drop.
//!
//! One freeze runs at a time, on one background task the server owns — a third
//! beside the fill's and the sync's. A second folder armed while one is running
//! is queued rather than dropped and rather than superseding it: a freeze is one
//! book being brought in and it commits one batch (spec: PK-7), so a book put
//! aside half way is one that was never brought in at all — its pages still in
//! the folder, not one of them an Entry, and the run on record the one that
//! displaced it. That is where this differs from the fill, which follows whoever
//! is clicking, and from the sync, which collapses because one walk of the
//! mappings finds everything. A folder that is already being frozen, or already
//! waiting, is not queued twice: asking for the same book again is asking for
//! what is already happening.
//!
//! # What it is not allowed to do
//!
//! It runs [`freeze`](coffret_device::OpenLibrary::freeze) over that one folder
//! and nothing else — no second reading of what is eligible, and no target of
//! its own: the size a Pack comes out is
//! [`DEFAULT_PACK_TARGET`](coffret_device::DEFAULT_PACK_TARGET), the one the
//! command line defaults to, so a book packed from a browser and a book packed
//! from a terminal come out alike.
//!
//! And a run that returns `Ok` has still to be read for what it left alone
//! (spec: PK-14, EP-12). Those findings reach the browser as
//! [`Noted`](crate::Noted), the same shape a sync's do: somebody who dropped a
//! book is owed the answer that a page of it was not packed, and they are not at
//! a terminal to be told there.

mod freeze_activity;
pub use freeze_activity::FreezeActivity;

mod freeze_folder;
pub use freeze_folder::freeze_folder;

mod freeze_status;
pub use freeze_status::FreezeStatus;

mod freezes;
pub use freezes::Freezes;

// Everything the server knows about freezing folders, in the one value the
// others read and write it through.
mod progress;

// One folder packed, from the files in it to the batch that commits them.
mod run;

// The background task itself, and what it puts back however it ends.
mod worker;
