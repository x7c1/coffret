//! Reading the Library as folders, out of the catalog alone.
//!
//! A Library has no folders. An Entry Path is one string whose only logical
//! separator is `/` (spec: EP-2), and the catalog stores exactly that — so
//! "what is in this folder" is an implication of the paths rather than
//! something anything recorded. This module draws that implication, and it
//! draws it in the device layer so that the explorer and anything else over
//! this crate read one answer rather than each inventing their own.
//!
//! Nothing here reaches Storage. Every question is about which Entries the
//! Library currently holds and which of them this device has on disk, and both
//! are in the catalog (spec: CK-7, EP-10). A listing therefore costs no network
//! and works on a device whose provider is unreachable.
//!
//! # Why this is a range scan and not an Index method
//!
//! The [`Index`](coffret_usecase::Index) has no folder concept and no
//! "children of" query, and this does not add one. What it uses is
//! [`entries_under`](coffret_usecase::Index::entries_under), which is a range
//! scan of the whole subtree, and it derives the children from that in memory.
//!
//! That is deliberate rather than provisional. The range scan is what the
//! adapter's primary key already answers directly, and the measurement behind
//! the catalog — 3,300 rows in 24 ms — says one is nothing to a folder of any
//! size a person browses. A port method would need a SQLite implementation, an
//! in-memory one, and a conformance case, all in exchange for a speedup nobody
//! has measured a need for.
//!
//! The seam is here, though, and it is a small one: a Library of tens of
//! thousands of Entries under one prefix would make the range scan the wrong
//! shape, and the answer then is a `children_of` on the port with the two
//! `/`-counting rules the `list` module applies in memory moved into each
//! adapter's SQL. Until a measurement asks for it, one implementation in one
//! place is worth more than the query.

mod child_folder;
pub use child_folder::ChildFolder;

mod entry_state;
pub use entry_state::EntryState;

mod file_row;
pub use file_row::FileRow;

mod folder_listing;
pub use folder_listing::FolderListing;

// Every folder the Library's paths imply.
mod folders;

// One folder's immediate children, which is the only question here that reads
// more than one kind of catalog row.
mod list;

// Whether this device has one Entry's file.
mod state_of;

#[cfg(test)]
mod tests;
