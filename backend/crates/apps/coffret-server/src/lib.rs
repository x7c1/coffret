//! What the browser-based explorer reads a Library through.
//!
//! A shell, exactly as the command line is one. Every question it answers is a
//! call on `coffret-device` — what folders the Library has, what one of them
//! holds, and the plaintext of one Entry — so the browser and the terminal read
//! the same Library rather than two readings of it. No flow, no layout, and no
//! decision about where a Library lives is made here.
//!
//! # What the browser is told, and what it is not
//!
//! Three answers, all keyed by Entry Path: the folder tree, one folder's
//! immediate children, and one Entry's bytes. Nothing else crosses the
//! boundary — no key, no ciphertext, no Storage token, no grant — and Storage is
//! never listed on a browser's behalf. What the file route serves is plaintext
//! on this device's own disk: the file the mappings put the Entry at, fetched
//! into place first where this device did not already have it (spec: EP-9,
//! EP-10, EP-11).
//!
//! And one answer that is not about the Library at all. Fetching an Entry the
//! device does not have starts a [fill](Fills) of the folder around it, in the
//! background, because whoever opened page one is going to read page two; the
//! activity route says how far that has got. It is device state — work in
//! flight, gone when the process is, never uploaded.
//!
//! Something goes the other way as well, and in two shapes. Files dropped onto a
//! folder land in it and arm a [sync](Syncs), which is the gesture a person
//! makes at a file manager followed by the command they would have typed. Files
//! dropped onto a folder they have just made in the browser are a book being
//! brought in, and that arms a [freeze](Freezes) of that folder instead
//! (spec: PK-17): the pages go up once, as Packs, rather than as one Container
//! per page — which is what keeps a scanned book from costing hundreds of
//! Storage objects and hundreds of provider calls to open again.
//!
//! And one question that is about the Library and reaches Storage for no bytes:
//! [the refresh](refresh_catalog) replays the Journal records this device has
//! not seen into its catalog (spec: CK-9). The server asks it once as it starts,
//! and again whenever somebody presses refresh — never on a timer. Without it a
//! device that has just joined would serve an empty Library for as long as the
//! process ran, and one another device has committed past would serve the
//! Library as it was.
//!
//! The Passphrase is spent once, at startup, before anything is bound: one
//! process is one unlock, and the derived keys live as long as the process
//! (spec: DK-9). A Library that is not on this device, a Passphrase that does
//! not open it, and a grant that has run out are all refused there — with the
//! same words the command line uses, because they are the same refusals — rather
//! than becoming a server that answers every request with a failure.
//!
//! The startup catch-up is the one thing at that stage that is *not* fatal.
//! Reading what the Index already holds needs no Storage at all, and a server
//! that refused to start over an unreachable bucket would take the offline half
//! of the explorer down with the online half.
//!
//! # Loopback only
//!
//! The binary binds `127.0.0.1` and nothing else. There is no authentication on
//! these routes and there is not meant to be: whoever can reach the socket has
//! already been given the plaintext of the Library by the operating system's own
//! account boundary, and a port on another interface would be that plaintext
//! offered to whoever else is on the network.
//!
//! # A library and a binary
//!
//! The router is a value ([`router`]) over a state ([`ServerState`]) rather than
//! something the binary builds inline, so a case can drive it as the service it
//! is — no socket, no port to be free, and no ordering between cases.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod api_error;

mod classify;

mod entry_query;

mod fill;
pub use fill::{fill_folder, Activity, Declined, FillStatus, Fills};

mod folder;
pub use folder::Folder;

mod freeze;
pub use freeze::{freeze_folder, FreezeActivity, FreezeStatus, Freezes};

mod noted;
pub use noted::Noted;

mod refresh;
pub use refresh::{catch_up_at_startup, refresh_catalog, Refreshes};

mod reported;
pub use reported::Reported;

mod router;
pub use router::router;

mod routes;

mod state;
pub use state::ServerState;

mod sync;
pub use sync::{arm_sync, SyncActivity, SyncStatus, Syncs};

mod timestamp;
