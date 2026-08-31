//! The six things a browser may ask of a Library.
//!
//! Three of them are about what the Library holds and answer out of the
//! catalog alone; the fourth is the only one that reaches Storage to answer,
//! and it reaches it for one Entry at a time. There is deliberately no route
//! that lists Storage, and none that hands anything encrypted out: what crosses
//! this boundary is plaintext the device already has, or is about to place, and
//! nothing else.
//!
//! The last two are about the one piece of work nobody asked for — the fill that
//! brings over the rest of the folder somebody opened a file in. One says how
//! far it has got; one takes a folder up again after it was left unfinished —
//! Storage stopped it, or somebody clicking elsewhere took the fill away — which
//! arms Storage work rather than doing any of it while the request is open.
//! Neither is another way to ask for bytes.
//!
//! The three that name a place in the Library take it as `?path=`, for the
//! reason [`PathQuery`](crate::entry_query::PathQuery) gives.

mod activity;
pub use activity::activity;

mod file;
pub use file::file;

mod fill;
pub use fill::fill;

mod folders;
pub use folders::folders;

mod library;
pub use library::library;

mod list;
pub use list::list;
