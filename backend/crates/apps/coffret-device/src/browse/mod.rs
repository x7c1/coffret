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
//! # Why a listing asks the Index for a folder and not for a subtree
//!
//! A listing, the Library's totals and its list of folders were once derived
//! here out of [`entries_under`](coffret_usecase::Index::entries_under), a
//! range scan of the whole subtree, counted in memory. That costs every Entry
//! below the folder — all of them, for the root — and a Library of scanned
//! books reaches hundreds of thousands. So the Index answers these itself
//! ([`folder_contents`](coffret_usecase::Index::folder_contents),
//! [`library_summary`](coffret_usecase::Index::library_summary),
//! [`folders`](coffret_usecase::Index::folders)), and an adapter that keeps
//! each folder's totals beside the catalog answers out of the folder's own
//! rows, at a cost that does not grow with what lies below it (spec: LA-45).
//! What is left here is what only this device's mappings decide: whether a
//! folder is reachable, and which mapped top-level folders are empty.

mod child_folder;
pub use child_folder::ChildFolder;

// Which kind of Container holds one Entry, for a caller holding a path and no
// listing.
mod container_of;

// Which Entry stands at a path, for a caller telling whether a commit happened
// there since it last asked.
mod entry_at;

mod entry_state;
pub use entry_state::EntryState;

mod file_row;
pub use file_row::FileRow;

mod files_beside;
pub use files_beside::FilesBeside;

// The files either side of one row, in reading order.
mod beside;

mod folder_listing;
pub use folder_listing::FolderListing;

// Every folder the Library's paths imply.
mod folders;

// One folder's immediate children, which is the only question here that reads
// more than one kind of catalog row.
mod list;

// Which of one folder's Entries this device does not have.
mod remote_in;

// Whether this device has one Entry's file.
mod state_of;

// Whether this device has every Entry standing directly in one folder.
mod directly_present;

// Which rows a filtered tree keeps.
mod filter;

// What the whole Library comes to, for the details of the Library itself.
mod totals;
pub use totals::LibraryTotals;

#[cfg(test)]
mod tests;
