use std::error;
use std::fmt;
use std::path::PathBuf;

use coffret_model::Redacted;

use crate::local_io_error::LocalIoError;

/// Why a writer could not reach the folder one file belongs in, or could not
/// finish the file once it had.
///
/// The vocabulary of the [`Destinations`](crate::Destinations) capability, so
/// that the two writers that share it can each say what a refusal means in their
/// own words. A folder fetch meets [`Blocked`](Self::Blocked) while deciding
/// where it may write and reports that Entry as
/// [`Surfaced::UnreachablePlace`](crate::fetch::Surfaced::UnreachablePlace),
/// placing the rest; where there is no run to go on with — a placement whose
/// folder changed shape after the selection, and the explorer taking a dropped
/// file into a mapped folder — the same fence is
/// [`FetchError::UnmaterializablePath`](crate::fetch::FetchError::UnmaterializablePath).
/// Neither invents a second spelling for what the descent found (spec: EP-4).
///
/// The two variants are the whole of the distinction the layer above draws: a
/// path this device cannot materialize at all, and a disk that would not answer.
/// Which errno stood behind either is the gateway's to read and never a caller's
/// — the point of the capability is that no flow decides a verdict from an error
/// kind — so [`Io`](Self::Io) carries a [`LocalIoError`] whole rather than
/// spreading its three parts here: one refusal about a local file has one shape,
/// whichever capability reported it.
///
/// There is deliberately no `PartialEq`, for the reason the error types around
/// it have none: a caller decides from the variant and the fields it names.
#[derive(Debug)]
pub enum DescentError {
    /// Something on the way down is not a folder inside the mapped root.
    ///
    /// A symbolic link, an ordinary file where a folder must be, or a name that
    /// became one of those while the descent was walking past it. Any of them
    /// means the Entry Path cannot be materialized *here*: following it would
    /// put bytes somewhere the mapped root does not stand for, which is the one
    /// thing a device may never do with a path another device committed
    /// (spec: EP-4, EP-11). The scan side refuses the mirror of this by not
    /// following links out of a mapped folder (spec: EP-8).
    Blocked {
        /// The component the descent stopped at.
        ///
        /// In the value and not in the message, for the reason
        /// [`LocalIoError`] keeps one there: a local path is one of the things
        /// that may never reach a log line (spec: EL-1).
        path: PathBuf,
    },
    /// A folder on the way down, or the file itself, could not be made, read,
    /// written, flushed, stamped, renamed, or removed.
    Io(LocalIoError),
}

impl fmt::Display for DescentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blocked { .. } => f.write_str(
                "a folder on the way to a file is not one inside the mapped root, \
                 so no file here can stand for the Entry Path",
            ),
            // The path stays out of the message and stays in the value, which is
            // what `LocalIoError`'s own rendering already does.
            Self::Io(refused) => write!(f, "{refused}"),
        }
    }
}

impl error::Error for DescentError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Blocked { .. } => None,
            Self::Io(refused) => Some(refused),
        }
    }
}

impl Redacted for DescentError {
    /// Which of the two refusals it is, and what the disk said where the disk is
    /// what refused.
    ///
    /// Neither variant may say more, which is why this exists at all: a caller
    /// outside this crate holds one of these and has a log line to write.
    /// [`Blocked`](Self::Blocked) is *identified* by the component the descent
    /// stopped at, and that is a local path — so the variant is the whole of
    /// what a log may carry, and the message says no more either: the component
    /// stays in the value, where the caller that has a person to answer takes
    /// it and names it in a message of its own
    /// ([`FetchError::UnmaterializablePath`](crate::fetch::FetchError::UnmaterializablePath)
    /// is what this becomes there). [`Io`](Self::Io) renders through
    /// [`LocalIoError`]'s own log-safe form, so one refusal about a local file
    /// reads the same in a log whichever capability reported it.
    fn redacted(&self) -> String {
        match self {
            Self::Blocked { .. } => "Descent::Blocked".to_owned(),
            Self::Io(refused) => format!("Descent::Io: {}", refused.redacted()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;
    use crate::local_operation::LocalOperation;

    // EL-1: neither the log line nor the message a person is shown names the
    // component the descent stopped at. It stays in the value, for the caller
    // that has somebody to answer with it.
    #[test]
    fn a_blocked_place_says_it_was_blocked_and_never_which_component() {
        let refused = DescentError::Blocked {
            path: PathBuf::from("/home/someone/albums/link"),
        };

        assert_eq!(refused.redacted(), "Descent::Blocked");
        assert!(refused.to_string().contains("mapped root"));
        assert!(
            !refused.to_string().contains("someone"),
            "the component stays in the value and out of the message",
        );
    }

    #[test]
    fn a_refused_call_says_what_it_was_doing_and_never_where() {
        let refused = DescentError::Io(LocalIoError::new(
            LocalOperation::Renaming,
            "/home/someone/albums/spring.jpg",
            io::Error::from(io::ErrorKind::PermissionDenied),
        ));

        assert_eq!(
            refused.redacted(),
            "Descent::Io: Local::Io(operation=renamed, kind=PermissionDenied)",
        );
        assert!(
            !refused.redacted().contains("someone"),
            "no part of a local path may reach a log line",
        );
    }
}
