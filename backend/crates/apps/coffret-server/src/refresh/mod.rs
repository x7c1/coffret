//! Finding out what the Library has become since this device last looked.
//!
//! Everything else this server answers comes out of the Index, and the Index is
//! a cache: it holds what this device has replayed and nothing newer. A device
//! that has just joined has replayed nothing at all, and one whose Library
//! another device has committed into since is a head behind — so an explorer over
//! either would be showing a Library that is not the Library, and would go on
//! showing it for as long as the process ran.
//!
//! *Refresh* is the browser's name for that and the route's. The domain's name
//! is *catch up*, which is what the flow is called from the device layer
//! downwards (spec: CK-9) — and what the Index concept calls a refresh is a
//! different act altogether: the bookkeeping a device does to its own catalog
//! after a commit of its own. Nothing here commits, so this module never reaches
//! that one.
//!
//! Two places ask, and deliberately no third — besides the one a reconnect
//! makes once a renewed grant lets Storage answer again, which is the refresh
//! a person would otherwise have pressed next, and the one an Entry fetch
//! makes when the Container its catalog names turns out to be gone from
//! Storage, which is a stale catalog showing itself.
//!
//! **As the server starts**, before anything is bound: what a first window shows
//! is then the Library rather than whatever this device happened to know last
//! time. A failure there is not fatal — browsing the Index needs no Storage, and
//! that half of the explorer is meant to work offline — and neither is a Storage
//! that says nothing at all, which is what the deadline on that run is for.
//!
//! Not fatal is not the same as not worth saying, which is what [`Catalog`] is
//! for: whichever of the two asked, what the catch-up came to is recorded on the
//! server's own state and answered to a browser with the rest of what this
//! process is doing. An explorer that listed nothing would otherwise show an
//! empty Library whether the Library is empty or this device never learnt what
//! is in it.
//!
//! **When somebody asks**, through `POST /api/refresh`. Nothing catches up on a
//! timer: a reader who wants what is new says so. What the explorer does on
//! its own is look — `GET /api/remote-head` compares the newest head Storage's
//! listing names with the catalog's, reads no object, and replays nothing — so
//! the control that asks can say there is something to ask for. Nothing about
//! the Library's correctness rests on hearing about a commit promptly — a stale
//! catalog is exactly what every writing flow's own catch-up fixes before it
//! does anything (spec: CK-9), and what an Entry fetch meets as a Container
//! that is no longer there, which [`when_stale`] answers.
//!
//! Short of that one case, an Entry fetch does not catch up, and that is the
//! point of these being the only places: a fetch answers from the catalog as it
//! stands, and the listing each of these walks is kept for the fetches after
//! it, so a reader turning pages costs Storage the parcels and nothing else.
//!
//! # One at a time
//!
//! Two replays at once would read every control object twice to learn one thing,
//! so a second caller waits for the first, then asks again — which is
//! [`Refreshes`]'s business to say.
//!
//! # What it is not allowed to do
//!
//! It runs [`catch_up`](coffret_device::OpenLibrary::catch_up) and nothing else.
//! No Container is read, no file is placed, and nothing is uploaded: the catalog
//! advances and the bytes go on arriving the way they already do, when somebody
//! opens a file and the fill follows them into the folder. The one thing it
//! finishes besides is this device's own bookkeeping: a batch this device
//! committed whose refresh never ran — a server killed between the two — is
//! completed by the catch-up after it, so the files that batch carried in read
//! as this device's from the first window on (spec: OC-7).

mod catch_up_at_startup;
pub use catch_up_at_startup::catch_up_at_startup;

mod refresh_catalog;
pub(crate) use refresh_catalog::catch_up_after_reconnect;
pub use refresh_catalog::refresh_catalog;

mod refreshes;
pub use refreshes::Refreshes;

mod standing;
pub use standing::{Catalog, Standing};

// The one catch-up every ask above reaches the Library through — the two, the
// reconnect's and an Entry fetch's — and the line it writes about itself.
mod run;

pub(crate) mod when_stale;
