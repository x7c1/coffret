use std::error;
use std::fmt;
use std::io;
use std::num::ParseIntError;
use std::path::PathBuf;

use tracing::metadata::ParseLevelError;

use crate::log_settings::{LOG_LEVEL, LOG_MAX_BYTES};

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// What can go wrong installing the sink.
///
/// All of it happens once, at startup, before anything is logged — which is
/// why none of it is reported by logging it. Each variant that has a cause
/// keeps it as the value it arrived as rather than as a rendering of it: this
/// crate exists to preserve what something actually answered, and an error of
/// its own that flattened its cause into a sentence would be the same loss in
/// miniature.
#[derive(Debug)]
pub enum Error {
    /// The log directory could not be created, or could not be written in.
    Directory {
        /// The directory that was being opened.
        path: PathBuf,
        /// What the operating system answered, kind and all.
        cause: io::Error,
    },
    /// Neither `XDG_STATE_HOME` nor `HOME` is set, so there is no state
    /// directory to default to and the caller has to name one.
    NoStateDirectory,
    /// [`LOG_LEVEL`] does not begin with a level.
    UnreadableLevel {
        /// The whole setting, level and targets together.
        value: String,
        /// What reading the level reported.
        cause: ParseLevelError,
    },
    /// [`LOG_LEVEL`] names a target, after the level, that is empty.
    ///
    /// Nothing is emitted under an empty target, so it is a setting somebody
    /// meant to say something with — a stray comma, or a variable that expanded
    /// to nothing — rather than one to honour.
    EmptyTarget {
        /// The whole setting, level and targets together.
        value: String,
    },
    /// [`LOG_MAX_BYTES`] is not a number of bytes.
    UnreadableCeiling {
        /// The value it was set to.
        value: String,
        /// What reading the number reported.
        cause: ParseIntError,
    },
    /// A subscriber is already installed in this process.
    ///
    /// Installing a second one would silently do nothing, so it is reported
    /// instead: two entry points both claiming the sink is a mistake in how the
    /// application was assembled.
    AlreadyInstalled,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Directory { path, cause } => {
                write!(f, "could not use the log directory at {path:?}: {cause}")
            }
            Self::NoStateDirectory => f.write_str(
                "neither XDG_STATE_HOME nor HOME is set; name a log directory explicitly",
            ),
            Self::UnreadableLevel { value, cause } => write!(
                f,
                "{LOG_LEVEL} is set to {value:?}, which does not begin with a level: {cause}",
            ),
            Self::EmptyTarget { value } => write!(
                f,
                "{LOG_LEVEL} is set to {value:?}, which names a target that is empty",
            ),
            Self::UnreadableCeiling { value, cause } => write!(
                f,
                "{LOG_MAX_BYTES} is set to {value:?}, which is not a number of bytes: {cause}",
            ),
            Self::AlreadyInstalled => {
                f.write_str("a subscriber is already installed in this process")
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Directory { cause, .. } => Some(cause),
            Self::UnreadableLevel { cause, .. } => Some(cause),
            Self::UnreadableCeiling { cause, .. } => Some(cause),
            Self::NoStateDirectory | Self::EmptyTarget { .. } | Self::AlreadyInstalled => None,
        }
    }
}
