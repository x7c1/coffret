//! The four things a browser may ask about a Library.
//!
//! Three of them are about what the Library holds and answer out of the
//! catalog alone; the fourth is the only one that may reach Storage, and it
//! reaches it for one Entry at a time. There is deliberately no route that lists
//! Storage, and none that hands anything encrypted out: what crosses this
//! boundary is plaintext the device already has, or is about to place, and
//! nothing else.
//!
//! The two that name a place in the Library take it as `?path=`, for the reason
//! [`PathQuery`](crate::entry_query::PathQuery) gives.

mod file;
pub use file::file;

mod folders;
pub use folders::folders;

mod library;
pub use library::library;

mod list;
pub use list::list;
