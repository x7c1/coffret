use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::credentials::{Credentials, LaunchTicket};

/// A launch file that was written, and the ticket it delivers (spec: LA-21).
///
/// Holding it holds the ticket, which is wiped as this is dropped. Dropping it
/// leaves the file where it is: the name may already hold the next file,
/// written over this one, which removing this one would take away.
pub struct LaunchFile {
    pub(super) path: PathBuf,
    pub(super) ticket: LaunchTicket,
}

impl LaunchFile {
    /// Where the file is: what a person, or the browser, is given to open.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether the ticket has served its turn — exchanged, expired, or retired
    /// with its generation — so that the file is to be removed (spec: LA-17,
    /// LA-21).
    pub fn is_spent(&self, credentials: &Credentials) -> bool {
        !credentials.is_live(&self.ticket)
    }

    /// Removes the file. One that is already gone is not an error.
    ///
    /// A file that could not be removed still holds a ticket that is spent
    /// once it was exchanged or has expired (spec: LA-21).
    pub fn remove(self) -> io::Result<()> {
        match fs::remove_file(&self.path) {
            Err(cause) if cause.kind() != io::ErrorKind::NotFound => Err(cause),
            _ => Ok(()),
        }
    }
}
