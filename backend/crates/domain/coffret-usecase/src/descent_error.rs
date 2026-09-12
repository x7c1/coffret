use std::error;
use std::fmt;
use std::path::PathBuf;

use coffret_model::Redacted;

use crate::local_io_error::LocalIoError;
use crate::refused_root::RootRefused;

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
/// The three variants are the whole of the distinction the layer above draws: a
/// path this device cannot materialize at all, a root that is not the root the
/// mapping was recorded against, and a disk that would not answer. Which errno
/// stood behind any of them is the gateway's to read and never a caller's — the
/// point of the capability is that no flow decides a verdict from an error
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
        /// The folder on this device the descent stopped at.
        ///
        /// In the value and not in the message, for the reason
        /// [`LocalIoError`] keeps one there: a local path is one of the things
        /// that may never reach a diagnostic event (spec: EL-1).
        stopped_at: PathBuf,
    },
    /// The mapped root is not the root the mapping was recorded against
    /// (spec: EP-13).
    ///
    /// Asked of the root handle the descent has just opened and before a single
    /// component below it is descended, so a placement never begins in a folder
    /// whose identity has not been held against the mapping's. Only a write asks
    /// — a look reads and places nothing — and nothing repairs what it finds:
    /// only recording the mapping ever writes or adopts a marker.
    ///
    /// The root travels in the value for the reason
    /// [`Blocked`](Self::Blocked)'s folder does, and the reason travels with
    /// it because the caller is what has somebody to answer: a folder fetch
    /// reports the mapping and goes on with the device's others, while a single
    /// writer fails the request it was given (spec: EP-11, EP-13).
    ///
    /// Which mapping it is about is not here, and cannot be: the capability is
    /// handed the root and the path's components apart (spec: EP-9), leaving
    /// no Entry Path to name the mapping by. A caller that puts this refusal in
    /// front of a person has to name the mapping all the same — EP-13 asks a
    /// refusal to name the mapping and the reason — so it takes that name from
    /// the row it descended through, which is what
    /// [`LocalPlace::prefix`](crate::fetch::LocalPlace::prefix) carries.
    Refused {
        /// The mapped root the refusal is about, for the caller that names it.
        root: PathBuf,
        /// Which of EP-13's cases it was.
        reason: RootRefused,
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
            // The root stays in the value, as the blocked folder does, and
            // the reason says what is wrong with the folder standing there.
            Self::Refused { reason, .. } => write!(
                f,
                "the mapped root is not the folder this mapping was recorded against: {reason}"
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
            // The marker's own refusal where that is what made it, so a chain
            // printed from here ends at what the file held rather than at the
            // root.
            Self::Refused { reason, .. } => match reason {
                RootRefused::MarkerMalformed { cause } => Some(cause),
                _ => None,
            },
            Self::Io(refused) => Some(refused),
        }
    }
}

impl Redacted for DescentError {
    /// Which of the two refusals it is, and what the disk said where the disk is
    /// what refused.
    ///
    /// Neither variant may say more, which is why this exists at all: a caller
    /// outside this crate holds one of these and has a diagnostic event to
    /// write. [`Blocked`](Self::Blocked) is *identified* by the folder the
    /// descent stopped at, and that is a local path — so the variant is the
    /// whole of what a log may carry, and the message says no more either: the
    /// folder stays in the value, where the caller that has a person to
    /// answer takes it and names it in a message of its own
    /// ([`FetchError::UnmaterializablePath`](crate::fetch::FetchError::UnmaterializablePath)
    /// is what this becomes there). [`Io`](Self::Io) renders through
    /// [`LocalIoError`]'s own log-safe form, so one refusal about a local file
    /// reads the same in a log whichever capability reported it.
    fn redacted(&self) -> String {
        match self {
            Self::Blocked { .. } => "Descent::Blocked".to_owned(),
            // The refusal itself names neither the root nor the identity either
            // side of the comparison carried, so it travels whole: which shape
            // the wrong folder took is the whole of what an event is for here.
            Self::Refused { reason, .. } => format!("Descent::Refused: {}", reason.redacted()),
            Self::Io(refused) => format!("Descent::Io: {}", refused.redacted()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;
    use crate::local_operation::LocalOperation;

    // EL-1: neither the diagnostic event nor the message a person is shown
    // names the folder the descent stopped at. It stays in the value, for
    // the caller that has somebody to answer with it.
    #[test]
    fn a_blocked_place_says_it_was_blocked_and_never_where_it_stopped() {
        let refused = DescentError::Blocked {
            stopped_at: PathBuf::from("/home/someone/albums/link"),
        };

        assert_eq!(refused.redacted(), "Descent::Blocked");
        assert!(refused.to_string().contains("mapped root"));
        assert!(
            !refused.to_string().contains("someone"),
            "the folder stays in the value and out of the message",
        );
    }

    // EP-13 and EL-1 together: the reason a root was refused is loggable and the
    // root is not, so the event carries which shape the wrong folder took and
    // the path stays in the value for whoever has a person to answer.
    #[test]
    fn a_refused_root_says_which_refusal_and_never_which_folder() {
        let refused = DescentError::Refused {
            root: PathBuf::from("/home/someone/albums"),
            reason: RootRefused::MarkerMismatch,
        };

        assert_eq!(refused.redacted(), "Descent::Refused: MarkerMismatch");
        assert!(
            !refused.redacted().contains("someone") && !refused.to_string().contains("someone"),
            "no part of a local path may reach an event or a message from here",
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
            "no part of a local path may reach a diagnostic event",
        );
    }
}
