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
            // The three variants with a cause name the directory or the
            // setting this layer was working from and stop there. What the
            // operating system or the reader answered is the value `source`
            // hands on, which a caller printing the chain reads under this
            // line — rendering it here as well would say one refusal twice.
            Self::Directory { path, .. } => {
                write!(f, "could not use the log directory at {path:?}")
            }
            Self::NoStateDirectory => f.write_str(
                "neither XDG_STATE_HOME nor HOME is set; name a log directory explicitly",
            ),
            Self::UnreadableLevel { value, .. } => write!(
                f,
                "{LOG_LEVEL} is set to {value:?}, which does not begin with a level",
            ),
            Self::EmptyTarget { value } => write!(
                f,
                "{LOG_LEVEL} is set to {value:?}, which names a target that is empty",
            ),
            Self::UnreadableCeiling { value, .. } => write!(
                f,
                "{LOG_MAX_BYTES} is set to {value:?}, which is not a number of bytes",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The links a caller printing `{error:#}` reads, outermost first.
    fn chain(error: &dyn error::Error) -> Vec<String> {
        let mut links = vec![error.to_string()];
        let mut below = error.source();
        while let Some(link) = below {
            links.push(link.to_string());
            below = link.source();
        }
        links
    }

    // This layer names the directory it was working from; what the operating
    // system said is the link under it, and the chain holds each of them once.
    #[test]
    fn a_refused_log_directory_reaches_a_caller_as_two_different_sentences() {
        let error = Error::Directory {
            path: PathBuf::from("/home/someone/.local/state/coffret/logs"),
            cause: io::Error::from(io::ErrorKind::PermissionDenied),
        };

        assert_eq!(
            chain(&error),
            vec![
                "could not use the log directory at \
                 \"/home/someone/.local/state/coffret/logs\""
                    .to_owned(),
                "permission denied".to_owned(),
            ],
        );
    }

    // The same of a setting this layer read: it says which variable was set to
    // what, and what reading it reported stays the reader's own sentence.
    #[test]
    fn an_unreadable_ceiling_reaches_a_caller_as_two_different_sentences() {
        let cause = "not a number"
            .parse::<u64>()
            .expect_err("that is no number");
        let error = Error::UnreadableCeiling {
            value: "not a number".to_owned(),
            cause: cause.clone(),
        };

        assert_eq!(
            chain(&error),
            vec![
                format!(
                    "{LOG_MAX_BYTES} is set to \"not a number\", which is not a number of bytes"
                ),
                cause.to_string(),
            ],
        );
    }
}
