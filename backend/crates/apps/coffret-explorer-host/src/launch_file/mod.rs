//! The file a launch ticket reaches the browser through (spec: LA-21).
//!
//! A ticket is never on a command line, in an environment variable or in an
//! HTTP request: the browser's command line is something another account on
//! the device can read. What reaches the browser instead is the path of a file
//! that only the account owning the Library can read, and the file is a page
//! that refreshes to `http://127.0.0.1:<port>/#launch=<ticket>`, which the
//! explorer's page then takes the ticket out of.
//!
//! [`LaunchDir`] is the owner-only directory the files are kept in, and
//! [`LaunchDir::deliver`] issues a ticket and writes it there as a
//! [`LaunchFile`]: created exclusively under a temporary name at mode `0600`
//! and renamed into place, so that nothing ever reads half of one and nobody
//! else can read one at all. Whoever delivered it removes it once its ticket
//! is spent — exchanged, expired, or retired with its generation — which
//! [`LaunchFile::is_spent`] says.
//!
//! The development host (`coffret-explorer-dev`) is what writes them: it opens
//! the explorer in a system browser, which has no other way to be handed a
//! ticket. The desktop app writes none; it loads the explorer into a window of
//! its own with the ticket in the URL's fragment.
//!
//! What the file holds is a page with no executable part: a refresh, a link
//! for a browser that does not follow one, and nothing else.
//!
//! Unix only: the modes and the owner are what make the file a boundary.

mod content;

mod launch_dir;
pub use launch_dir::LaunchDir;

mod launch_file_error;
pub use launch_file_error::LaunchFileError;

mod written;
pub use written::LaunchFile;

#[cfg(test)]
mod tests;

/// The mode a launch file is created at: its owner's alone (spec: LA-21).
const FILE_MODE: u32 = 0o600;

/// The mode the directory is created at, and the widest it may be found at.
const DIRECTORY_MODE: u32 = 0o700;
