//! The eight things a browser may ask of a Library.
//!
//! Three of them are about what the Library holds and answer out of the
//! catalog alone; the fourth is the only one that reaches Storage to answer,
//! and it reaches it for one Entry at a time. There is deliberately no route
//! that lists Storage, and none that hands anything encrypted out: what crosses
//! this boundary is plaintext the device already has, or is about to place, and
//! nothing else.
//!
//! One goes the other way. The upload takes files somebody dropped into the
//! folder this device maps and arms a sync over them, which is the same gesture
//! as copying them in and typing `coffret sync` — it is where a Library gains
//! anything through the explorer, and it gains it through the flow the command
//! line uses rather than through a second one.
//!
//! The last three are about the work nobody asked for — the fill that brings
//! over the rest of the folder somebody opened a file in, and the sync that
//! carries in what they dropped. One says how far both have got; the other two
//! take one up again after it was left unfinished — Storage stopped it, or
//! somebody clicking elsewhere took the fill away — which arms Storage work
//! rather than doing any of it while the request is open. None of the three is
//! another way to ask for bytes.
//!
//! Those that name a place in the Library take it as `?path=`, for the reason
//! [`PathQuery`](crate::entry_query::PathQuery) gives.

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

mod sync;
pub use sync::sync;

mod upload;
pub use upload::upload;
