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
//! Two places ask, and deliberately no third.
//!
//! **As the server starts**, before anything is bound: what a first window shows
//! is then the Library rather than whatever this device happened to know last
//! time. A failure there is not fatal — browsing the Index needs no Storage, and
//! that half of the explorer is meant to work offline — and neither is a Storage
//! that says nothing at all, which is what the deadline on that run is for.
//!
//! **When somebody asks**, through `POST /api/refresh`. There is no polling and
//! no following of the remote head: the explorer's discipline is zero requests
//! while nothing is happening, and a reader who wants to know what is new says
//! so. Nothing about the Library's correctness rests on hearing about a commit
//! promptly — a stale catalog is exactly what every flow's own catch-up fixes
//! before it does anything (spec: CK-9).
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
//! opens a file and the fill follows them into the folder.

mod catch_up_at_startup;
pub use catch_up_at_startup::catch_up_at_startup;

mod refresh_catalog;
pub use refresh_catalog::refresh_catalog;

mod refreshes;
pub use refreshes::Refreshes;

// The one catch-up both of the two asks reach the Library through, and the line
// it writes about itself.
mod run;
