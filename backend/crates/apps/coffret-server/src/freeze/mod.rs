//! Packing a book somebody just brought in, without them asking.
//!
//! A scanned book is one folder of a few hundred page images, and carrying it in
//! the way a dropped photograph is carried in would make it a few hundred
//! Storage Objects — a few hundred uploads, a few hundred provider calls to open
//! it again, and a few hundred more for every rebuild after that. `freeze` is
//! the flow that puts those pages into Packs instead (spec: PK-1, PK-7), and
//! until now the only way to reach it was the command line.
//!
//! # Asked for by the drop, and one book at a time
//!
//! There is no "pack this" button and this is not one. What arms a freeze is a
//! drop holding a folder, once the person has been asked and chose to add it as
//! a Pack: a folder of pages arriving in one gesture is a book being imported,
//! and they have said so. `POST /api/freeze` exists for what that trigger
//! cannot reach — a freeze Storage stopped, with the pages already sitting in
//! the folder and nothing left to drop — exactly as `POST /api/fill` and
//! `POST /api/sync` do.
//!
//! What a drop's freeze packs is what the drop carried, and not the folder it
//! went into: the run is armed with the Entry Paths the drop wrote as its
//! selection (see [`Book`]). A folder the Library already has may hold one-file
//! Entries, which are eligible for a freeze (spec: PK-1); the selection is what
//! leaves them where they are, so what the person was shown before the drop and
//! what gets packed are the same files (spec: PK-17). Existing Packs are never a
//! freeze's inputs either way (spec: PK-1, PK-2), and regrouping across
//! invocations is repack's and compaction's work (spec: PK-8).
//!
//! One freeze runs at a time, on one worker the server owns — a third
//! beside the fill's and the sync's. A second folder armed while one is running
//! is queued rather than dropped and rather than superseding it: a freeze is one
//! book being brought in and it commits one batch (spec: PK-7), so a book put
//! aside half way is one that was never brought in at all — its pages still in
//! the folder, not one of them an Entry, and the run on record the one that
//! superseded its freeze. The fill queues a folder asked for by name in the
//! same way, and parts from this only where a fetch lands somewhere else and it
//! follows whoever is clicking; the sync parts from it outright, collapsing
//! because one walk of the mappings finds everything. A book whose files the
//! running freeze is already packing is not queued again: asking for the same
//! book again is asking for what is already happening. And a second drop into a
//! folder already waiting joins that book, so one run packs both drops' files.
//!
//! # What it is not allowed to do
//!
//! It runs [`freeze`](coffret_device::OpenLibrary::freeze) over that one book
//! and nothing else — no second reading of what is eligible, and no target of
//! its own: the size a Pack comes out is
//! [`DEFAULT_PACK_TARGET`](coffret_device::DEFAULT_PACK_TARGET), the one the
//! command line defaults to, so a book packed from a browser and a book packed
//! from a terminal come out alike.
//!
//! And a run that returns `Ok` has still to be read for what it left alone
//! (spec: PK-14, EP-12). Those findings reach the browser as
//! [`Finding`](crate::Finding), the same shape a sync's do: somebody who dropped a
//! book is owed the answer that a page of it was not packed, and they are not at
//! a terminal to be told there.

mod book;
pub use book::Book;

mod freeze_run;
pub use freeze_run::FreezeRun;

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

// The worker itself, and what it puts back however it ends.
mod worker;
