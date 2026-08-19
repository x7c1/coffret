//! The directory a fixture set lives in.
//!
//! ```text
//! <dir>/manifest.json      what the set states about itself
//! <dir>/objects/<name>     Storage Objects, under the names they are stored as
//! <dir>/blobs/<name>       byte strings that are not Storage Objects
//! ```
//!
//! Relative paths in the manifest are the only way in: a reader opens what the
//! manifest points at rather than guessing a layout, so a set stays readable
//! even if a future fixture needs a place of its own.

mod fixture_reader;
pub use fixture_reader::FixtureReader;

mod fixture_writer;
pub use fixture_writer::FixtureWriter;

/// Where Storage Objects live inside a fixture set.
pub const OBJECTS_DIR: &str = "objects";

/// Where byte strings that never reach Storage live inside a fixture set.
pub const BLOBS_DIR: &str = "blobs";
