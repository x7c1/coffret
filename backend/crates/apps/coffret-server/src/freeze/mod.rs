//! Packing a book somebody just brought in, or a folder they asked to have
//! packed.
//!
//! A scanned book is one folder of a few hundred page images, and carrying it in
//! the way a dropped photograph is carried in would make it a few hundred
//! Storage Objects — a few hundred uploads, a few hundred provider calls to open
//! it again, and a few hundred more for every rebuild after that. `freeze` is
//! the flow that puts those pages into Packs instead (spec: PK-1, PK-7).
//!
//! # Asked for by a drop, or by the folder's own button
//!
//! Two gestures arm a freeze. A drop holding a folder, once the person has been
//! asked and chose to add it as a Pack: a folder of pages arriving in one
//! gesture is a book being imported, and they have said so. And "Pack this
//! folder…" over a folder this device maps, for a book that is already in the
//! Library one file at a time — added one by one, or synced from the mapped
//! folder — which `POST /api/freeze` arms once the person has been shown what
//! `GET /api/freeze` counted. The same `POST` takes up a freeze Storage
//! stopped, with the pages already sitting in the folder and nothing left to
//! drop, exactly as `POST /api/fill` and `POST /api/sync` take theirs up.
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

/// Whether `book`'s folder is being packed already: the run under way packs
/// everything it asks for, or its folder is waiting its turn.
///
/// What the preview tells the browser, so that the folder's question does not
/// promise a run after the current one that the `POST` would not arm. The run
/// on record counts only while it is freezing and only where it covers the
/// book: a drop's run packs only the files the drop carried, so the whole
/// folder asked for beside it is queued as a book of its own. A waiting book
/// counts by its folder alone, because the queue names no more than that and
/// the `POST` joins the book waiting there rather than queueing a second.
/// Where that waiting book is a drop's, the `POST` would have widened it to
/// the whole folder; a press once that run has finished packs the rest.
pub fn packs_already(freezes: &Freezes, book: &Book) -> bool {
    let Some(latest) = freezes.reported() else {
        return false;
    };
    let run = &latest.on_record;
    let under_way = matches!(run.status, FreezeStatus::Freezing)
        && Book {
            folder: run.folder.clone(),
            only: run.only.clone(),
        }
        .covers(book);
    under_way || latest.waiting.contains(&book.folder)
}
