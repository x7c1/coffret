//! A drop of files and folders by their paths on this device, held for the
//! one import that carries it in.
//!
//! In the desktop app the explorer runs in the app's own window, so what a
//! person lets go of over it can be handed over as paths rather than read into
//! a browser and sent as bytes. The app hands those paths here
//! ([`walk_drop`]), which walks them through the scan's own confined listing
//! (spec: EP-8) and holds what it took under a one-time **drop handle**
//! ([`Drops`]). What goes back to the page is that handle and, per file, its
//! path relative to the folder the dropped item stood in, its size and its
//! modification time ([`Dropped`]) — never a path on this device.
//!
//! The import then names the handle and, per file, its index in the held set
//! and the name it is to have in the Library. The server takes the set out of
//! [`Drops`], which forgets it as it hands it over, and reads each file it is
//! asked for through the same confined reader a freeze reads a source through
//! ([`DroppedFile::open`]), so nothing that arrives over HTTP ever names a
//! path on this device.

mod drop_reader;
pub use drop_reader::DropReader;

mod drop_refusal;
pub use drop_refusal::DropRefusal;

mod dropped;
pub use dropped::Dropped;

mod dropped_file;
pub use dropped_file::DroppedFile;

mod dropped_row;
pub use dropped_row::DroppedRow;

mod dropped_set;
pub use dropped_set::DroppedSet;

mod held_drops;
pub use held_drops::{Drops, DROP_LIFETIME, HELD_DROPS};

mod not_taken_row;
pub use not_taken_row::NotTakenRow;

mod walk_drop;
pub use walk_drop::walk_drop;

#[cfg(test)]
mod tests;
