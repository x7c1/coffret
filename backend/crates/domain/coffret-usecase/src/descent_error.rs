use std::error;
use std::fmt;
use std::path::PathBuf;

use coffret_model::Redacted;

use crate::below_root_error::BelowRootError;
use crate::local_io_error::LocalIoError;
use crate::root_refused::RootRefused;

/// Why a writer could not reach the folder one file belongs in.
///
/// The vocabulary of [`Destinations::reach`](crate::Destinations::reach), which
/// is the one call that holds a mapped root against the identity its mapping
/// recorded and so the one that needs a word for a root that is not the recorded
/// one. No other step holds a root that way — a look places nothing, and a
/// placement's own calls are made against the folder that reach left open — so
/// they fail in [`BelowRootError`](crate::BelowRootError) instead, which is
/// these same two ways about the path without the two that only the root's own
/// question raises.
///
/// The two writers that share the capability each say what a refusal means in
/// their own words, and a reach is made where there is no run to go on with —
/// a placement whose folder changed shape after the selection, and the explorer
/// taking a dropped file into a mapped folder — so [`Blocked`](Self::Blocked) is
/// [`FetchError::UnmaterializablePath`](crate::fetch::FetchError::UnmaterializablePath)
/// for both of them. Neither invents a second spelling for what the descent
/// found (spec: EP-4). A folder fetch meets the same fence one call earlier,
/// while its selection is deciding where it may write, and that call is a look:
/// it answers in [`BelowRootError`](crate::BelowRootError), and the Entry is
/// reported as
/// [`Surfaced::UnreachablePlace`](crate::fetch::Surfaced::UnreachablePlace) with
/// the rest of the run placed.
///
/// The four variants are the whole of the distinction the layer above draws: a
/// path this device cannot materialize at all, a root that is not the root the
/// mapping was recorded against, a disk that would not answer the question the
/// root's identity is asked with, and a disk that would not answer somewhere on
/// one file's own way down. The last two are one failure told apart by what it
/// is about rather than by what went wrong, which is the whole of why
/// [`Unvouched`](Self::Unvouched) is a variant: a caller handed several
/// placements through one root can go on past an [`Io`](Self::Io) and has read
/// the last of what it can place once it meets an `Unvouched`. Which errno
/// stood behind any of them is the gateway's to read and never a caller's — the
/// point of the capability is that no flow decides a verdict from an error
/// kind — so both carry a [`LocalIoError`] whole rather than spreading its three
/// parts here: one refusal about a local file has one shape, whichever
/// capability reported it.
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
    /// The operating system would not answer the question the root's identity
    /// is asked with (spec: EP-13).
    ///
    /// Not a [`Refused`](Self::Refused), and deliberately: a permission the
    /// process has not on the management area says nothing about which folder
    /// this is, and reading it as a mismatch would send a person to record the
    /// mapping again over something that is not about the mapping at all. So
    /// what travels is what the operating system said, exactly as
    /// [`Io`](Self::Io) carries it.
    ///
    /// Its own variant all the same, because of what it is *about*. The marker
    /// stands in the mapped root every placement through that root goes
    /// through, so an answer that did not come is settled for all of them
    /// before the first one is written: a caller handed several — one upload's
    /// files are that (spec: EP-11) — has nothing left to place through this
    /// mapping, where a refusal met below the root costs one file and leaves
    /// the next alone.
    Unvouched {
        /// The mapped root the answer was wanted about, for the caller that
        /// names it.
        ///
        /// In the value and not in the message, for the reason
        /// [`Blocked`](Self::Blocked)'s folder is.
        root: PathBuf,
        /// What the operating system said, and which of the root's own names it
        /// said it about.
        cause: LocalIoError,
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
            // The root stays in the value here too, and the sentence says the
            // one thing this refusal settles that the one below does not: the
            // question about the mapping went unanswered, so nothing is known
            // about it either way.
            Self::Unvouched { cause, .. } => write!(
                f,
                "this device could not read the mapped root's own marker, so whether it is the \
                 folder this mapping was recorded against is unanswered: {cause}"
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
            Self::Unvouched { cause, .. } => Some(cause),
            Self::Io(refused) => Some(refused),
        }
    }
}

impl Redacted for DescentError {
    /// Which of the four refusals it is, and what the disk said where the disk
    /// is what refused.
    ///
    /// No variant may say more, which is why this exists at all: a caller
    /// outside this crate holds one of these and has a diagnostic event to
    /// write. [`Blocked`](Self::Blocked) is *identified* by the folder the
    /// descent stopped at, and that is a local path — so the variant is the
    /// whole of what a log may carry, and the message says no more either: the
    /// folder stays in the value, where the caller that has a person to
    /// answer takes it and names it in a message of its own
    /// ([`FetchError::UnmaterializablePath`](crate::fetch::FetchError::UnmaterializablePath)
    /// is what this becomes there). [`Io`](Self::Io) and
    /// [`Unvouched`](Self::Unvouched) render through [`LocalIoError`]'s own
    /// log-safe form, so one refusal about a local file reads the same in a log
    /// whichever capability reported it; which of the two it was is the variant,
    /// which is what an event has to group by — the two are answered differently
    /// and a log that spelled them alike could not say how often either arrives.
    /// That holds of the value a caller still has as one of these. A caller that
    /// answers them alike may hand one on as the other before anything is
    /// written, and a folder fetch's placement does: what reaches a log there is
    /// an [`Io`](Self::Io), for the reason that call gives.
    fn redacted(&self) -> String {
        match self {
            Self::Blocked { .. } => "Descent::Blocked".to_owned(),
            // The refusal itself names neither the root nor the identity either
            // side of the comparison carried, so it travels whole: which shape
            // the wrong folder took is the whole of what an event is for here.
            Self::Refused { reason, .. } => format!("Descent::Refused: {}", reason.redacted()),
            Self::Unvouched { cause, .. } => format!("Descent::Unvouched: {}", cause.redacted()),
            Self::Io(refused) => format!("Descent::Io: {}", refused.redacted()),
        }
    }
}

impl From<BelowRootError> for DescentError {
    /// The same refusal in the vocabulary a descent that also vouches for a root
    /// needs.
    ///
    /// Both ways below a vouched root are ways a `reach` can fail too — it walks
    /// the same components once the marker has agreed — so the wider type says
    /// them in the same words rather than in second ones of its own.
    fn from(refused: BelowRootError) -> Self {
        match refused {
            BelowRootError::Blocked { stopped_at } => Self::Blocked { stopped_at },
            BelowRootError::Io(refused) => Self::Io(refused),
        }
    }
}

impl From<LocalIoError> for DescentError {
    /// What the operating system refused, unchanged: nothing is decided on the
    /// way, so `?` carries one into the other.
    fn from(refused: LocalIoError) -> Self {
        Self::Io(refused)
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

    // EP-13 and EL-1 again, for the refusal that is about the root without
    // being a verdict on it: the event says the disk would not answer the
    // marker's question, and neither the event nor the message names the
    // folder. The identity is its own, so a reader can tell it from a refusal
    // met on one file's way down.
    #[test]
    fn a_root_that_could_not_be_asked_about_says_so_and_never_which_folder() {
        let refused = DescentError::Unvouched {
            root: PathBuf::from("/home/someone/albums"),
            cause: LocalIoError::new(
                LocalOperation::Reading,
                "/home/someone/albums/.coffret/root",
                io::Error::from(io::ErrorKind::PermissionDenied),
            ),
        };

        assert_eq!(
            refused.redacted(),
            "Descent::Unvouched: Local::Io(operation=read, kind=PermissionDenied)",
        );
        assert!(
            !refused.redacted().contains("someone") && !refused.to_string().contains("someone"),
            "no part of a local path may reach an event or a message from here",
        );
        assert!(
            !refused.to_string().contains("is not the folder"),
            "a disk that would not answer is not a folder told it is the wrong one: {refused}",
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

    // The wider vocabulary says the same two in the same words, which is what
    // lets a `reach` walk the components of an already-vouched root and report
    // what it meets without a second spelling of it.
    #[test]
    fn a_step_below_a_vouched_root_reads_the_same_as_a_descent() {
        let blocked = DescentError::from(BelowRootError::Blocked {
            stopped_at: PathBuf::from("/home/someone/albums/link"),
        });
        assert_eq!(blocked.redacted(), "Descent::Blocked");

        let refused = DescentError::from(BelowRootError::Io(LocalIoError::new(
            LocalOperation::Renaming,
            "/home/someone/albums/spring.jpg",
            io::Error::from(io::ErrorKind::PermissionDenied),
        )));
        assert_eq!(
            refused.redacted(),
            "Descent::Io: Local::Io(operation=renamed, kind=PermissionDenied)",
        );
    }
}
