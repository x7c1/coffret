//! What a drop of files and folders by their paths on this device carries in.
//!
//! A drop onto the explorer in the desktop app arrives as the paths of what
//! was let go — a few files, a folder, a folder of folders — rather than as
//! bytes a browser read. This walks them: every regular file under a dropped
//! folder, depth first, and each dropped file itself, each with the path it
//! takes relative to the folder the dropped item stood in.
//!
//! It is the scan's listing and nothing new: every folder is read through
//! [`MappedRoots::list_folder`], which descends from the folder the dropped
//! item stood in without following a link and states each child with links
//! unfollowed (spec: EP-8). The folder the item stood in is taken as the root,
//! as a mapped root is, and nothing above it is walked. A symbolic link is
//! therefore never followed and never taken, whether it points at a file or at
//! a folder, and it is reported rather than left out without a word. So is a
//! name coffret keeps for itself inside a mapped folder ([`reserved_name`],
//! spec: EP-11, EP-14), which a file written under would never reach the
//! Library, and anything the walk could not read.
//!
//! What it does not decide is where the files go, or whether the Library will
//! have them there: that is the import's, which places each one through the
//! same writer a browser's drop goes through (spec: EP-9, EP-11).
//!
//! [`MappedRoots::list_folder`]: crate::MappedRoots::list_folder
//! [`reserved_name`]: crate::reserved_names::reserved_name

mod dropped_item;
pub use dropped_item::DroppedItem;

mod dropped_source;
pub use dropped_source::DroppedSource;

mod not_taken;
pub use not_taken::NotTaken;

mod not_taken_reason;
pub use not_taken_reason::NotTakenReason;

mod walk_dropped;
pub use walk_dropped::walk_dropped;

/// Everything one drop carried.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DropWalk {
    /// Every regular file taken, depth first.
    pub taken: Vec<DroppedSource>,
    /// Everything met and not taken, in the order it was met.
    pub not_taken: Vec<NotTaken>,
}
