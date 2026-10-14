use std::fmt;
use std::io;
use std::net::SocketAddr;

use crate::credentials::NotDrawn;

/// A launch file that was not written, and why.
///
/// No variant names the file or the directory, and none carries the ticket:
/// the caller knows where it asked for the file, and says so where a person
/// is told (spec: EL-1).
#[derive(Debug)]
pub enum LaunchFileError {
    /// What the directory was asked to be is not a directory, or is a link to
    /// one.
    NotADirectory,
    /// The directory belongs to another account, which could read every
    /// ticket put in it.
    ForeignOwner,
    /// The directory admits somebody besides its owner: its mode, which must
    /// grant nothing to the group or to others.
    TooOpen {
        /// The permission bits it was found with.
        mode: u32,
    },
    /// The name asked for is not one file name ending in `.html`.
    NotAName,
    /// The host's address is not on `127.0.0.1`, which is the only address the
    /// host serves (spec: LA-13).
    NotLoopback(SocketAddr),
    /// No ticket could be drawn.
    NotDrawn(NotDrawn),
    /// The file system refused a step.
    Io {
        /// The step, in a fixed word.
        operation: &'static str,
        cause: io::Error,
    },
}

impl LaunchFileError {
    pub(super) fn io(operation: &'static str) -> impl FnOnce(io::Error) -> Self {
        move |cause| Self::Io { operation, cause }
    }
}

impl fmt::Display for LaunchFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotADirectory => {
                f.write_str("the place for launch files is not a directory of its own")
            }
            Self::ForeignOwner => f.write_str(
                "the directory for launch files belongs to another account, which could read \
                 the tickets in it",
            ),
            Self::TooOpen { mode } => write!(
                f,
                "the directory for launch files is mode {mode:o}, which lets others in; it has \
                 to be 700"
            ),
            Self::NotAName => {
                f.write_str("a launch file is named by one file name ending in .html")
            }
            Self::NotLoopback(address) => write!(
                f,
                "a launch file opens the explorer on 127.0.0.1 alone, and was asked to open \
                 {address}"
            ),
            Self::NotDrawn(_) => f.write_str("no launch ticket could be drawn"),
            Self::Io { operation, .. } => {
                write!(f, "a launch file could not be written: {operation} failed")
            }
        }
    }
}

impl std::error::Error for LaunchFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotDrawn(cause) => Some(cause),
            Self::Io { cause, .. } => Some(cause),
            _ => None,
        }
    }
}
