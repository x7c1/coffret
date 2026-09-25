//! Bringing over the rest of the folder somebody just opened a file in.
//!
//! Opening one `remote` image fetches exactly that image. For a book that is
//! the wrong shape: whoever opened page one is going to read page two, and the
//! rest of the Pack is meant to be brought over by the same mechanism as the
//! reader's own prefetch — in the background, unasked. The reader's prefetch
//! papers over it for the pages next to the one on the screen, but it stops when
//! the reader closes and leaves a half-`remote` folder behind.
//!
//! # Implicit, and one at a time
//!
//! There is no download button, and this is not one. What arms a fill is a
//! `GET /api/file` that had to fetch — the verdict was a placement rather than
//! `AlreadyPresent` — and the folder holding that Entry is what gets filled.
//! `POST /api/fill` exists for what the implicit trigger cannot reach: taking a
//! folder up again after it was left unfinished.
//!
//! One fill runs at a time, on one worker the server owns. There is
//! nothing like it on the command line, and there should not be: a one-shot
//! process has nobody left to fill for by the time it could.
//!
//! A second folder a fetch lands in does not queue behind the first — it
//! replaces it. Somebody who clicked into another folder has moved on, so the
//! fill follows them; the folder it left is not resumed on its own, and clicking
//! back into it arms it afresh.
//!
//! A folder somebody asks for by name is the other case, and it queues. What
//! reaches `POST /api/fill` is a button pressed on purpose — the retry beside a
//! fill that stopped, and one per folder a worker that died threw away — and
//! latest wins there would be the second press taking the first folder's line,
//! its button and every trace of it off the screen with half of it brought
//! over. Two presses bring both folders over, in the order they were pressed.
//!
//! # What it is not allowed to do
//!
//! It fetches through [`EntryFetches`](coffret_device::EntryFetches), the same
//! per-Entry gate the routes fetch through, so a reader's prefetch and this
//! never both place one Entry. And it stops for Storage and for nothing else: a
//! declined Entry is about that Entry alone (spec: EP-11), recorded so the
//! browser can mark the row, and the fill goes on to the next file — exactly as
//! the command line's `fetch` does.
//!
//! What it reports of itself is device state and nothing more, which is
//! [`Activity`]'s own business to say.

mod activity;
pub use activity::Activity;

mod declined;
pub use declined::Declined;

mod fill_folder;
pub use fill_folder::{fill_folder, queue_folder};

mod fill_status;
pub use fill_status::FillStatus;

mod fills;
pub use fills::Fills;

// Everything the server knows about filling folders, in the one value the
// others read and write it through.
mod progress;

// One folder brought over, from its listing to its last Entry.
mod run;

// The worker itself, and what it puts back however it ends.
mod worker;
