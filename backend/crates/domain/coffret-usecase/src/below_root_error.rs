use std::error;
use std::fmt;
use std::path::PathBuf;

use coffret_model::Redacted;

use crate::local_io_error::LocalIoError;

/// Why a step taken *below a root that has already been vouched for* could not
/// be taken.
///
/// Everything the [`Destinations`](crate::Destinations) capability does apart
/// from [`reach`](crate::Destinations::reach) itself: descending the folders of
/// a look, making a scratch, writing it, flushing it, stamping it, publishing
/// it, removing it. None of them asks the one question a root is asked at a
/// placement's start: a placement's own calls are made against the folder
/// `reach` left open, once it has been asked and answered, and a look places
/// nothing, so it is never asked at all. That is the whole of the difference
/// from [`DescentError`](crate::DescentError): `reach` holds the mapped root's
/// marker against the identity its mapping recorded and needs a word for a
/// root that is not the recorded one (spec: EP-13), and nothing below it does,
/// so nothing below it names one.
///
/// The two ways left are the two that are about the path rather than the
/// mapping. [`Blocked`](Self::Blocked) is a name on the way that is not a real
/// folder of the mapped root — a symbolic link, an ordinary file where a folder
/// must be, or a name that became one of those while the walk went past it — and
/// means the Entry Path cannot be materialized *here*, whatever the link points
/// at (spec: EP-4, EP-11). [`Io`](Self::Io) is the operating system refusing for
/// any other reason, carried whole as the [`LocalIoError`] it was reported in:
/// which errno stood behind either is the gateway's to read and never a
/// caller's.
///
/// It converts into [`DescentError`](crate::DescentError), so a `reach` that
/// meets one of these while walking reports it in the wider vocabulary without
/// restating what travels inside it: [`Blocked`](Self::Blocked) in these very
/// words, [`Io`](Self::Io) in a sentence of the descent's own, since a step on
/// the way to a root is not a step below one already reached, and the same
/// [`LocalIoError`] carried on untouched either way. One value comes the other
/// way and is the exception to the paragraph above: a fetch that meets
/// [`DescentError::Unvouched`](crate::DescentError::Unvouched) — the root's own
/// question left unanswered — hands what the operating system said to
/// [`Io`](Self::Io), because that run stops at the first refusal whichever it
/// was and a second spelling of the disk's answer would buy nothing. What
/// travels is still the disk's answer and never a verdict about the root — and
/// that one value is the exception to this variant's own sentence too, which is
/// written for a step taken below a root already reached. It costs nothing,
/// because the placement that makes one hands it to the fetch's vocabulary,
/// which takes the [`LocalIoError`] straight back out rather than reading a
/// sentence off the value it came in.
///
/// There is deliberately no `PartialEq`, for the reason the error types around
/// it have none: a caller decides from the variant and the fields it names.
#[derive(Debug)]
pub enum BelowRootError {
    /// Something on the way down is not a folder inside the mapped root.
    ///
    /// A symbolic link, an ordinary file where a folder must be, or a name that
    /// became one of those while the walk was passing it. Following it would put
    /// bytes somewhere the mapped root does not stand for, which is the one
    /// thing a device may never do with a path another device committed
    /// (spec: EP-4, EP-11).
    Blocked {
        /// The folder on this device the walk stopped at.
        ///
        /// In the value and not in the message, for the reason
        /// [`LocalIoError`] keeps one there: a local path is one of the things
        /// that may never reach a diagnostic event (spec: EL-1).
        stopped_at: PathBuf,
    },
    /// A folder on the way down, or the file itself, could not be made, read,
    /// written, flushed, stamped, renamed, or removed.
    Io(LocalIoError),
}

impl fmt::Display for BelowRootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blocked { .. } => f.write_str(
                "a folder on the way to a file is not one inside the mapped root, \
                 so no file here can stand for the Entry Path",
            ),
            // Where the step was taken, and nothing of the refusal itself:
            // this layer is the one that knows the step was below a mapped
            // root rather than on the way to one. `source` hands the
            // capability's refusal on whole, path and all, and that value says
            // which operation the disk refused and what it answered.
            Self::Io(_) => {
                f.write_str("a step below the mapped root could not be taken on this device")
            }
        }
    }
}

impl error::Error for BelowRootError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Blocked { .. } => None,
            Self::Io(refused) => Some(refused),
        }
    }
}

impl Redacted for BelowRootError {
    /// Which of the two refusals it is, and what the disk said where the disk is
    /// what refused.
    ///
    /// The reading [`DescentError`](crate::DescentError) makes of the same two,
    /// and for the same reason: [`Blocked`](Self::Blocked) is *identified* by
    /// the folder the walk stopped at and that is a local path, so the variant
    /// is the whole of what a log may carry and the folder stays in the value
    /// for the caller that has a person to answer (spec: EL-1).
    /// [`Io`](Self::Io) renders through [`LocalIoError`]'s own log-safe form, so
    /// one refusal about a local file reads the same in a log whichever
    /// capability reported it.
    fn redacted(&self) -> String {
        match self {
            Self::Blocked { .. } => "BelowRoot::Blocked".to_owned(),
            Self::Io(refused) => format!("BelowRoot::Io: {}", refused.redacted()),
        }
    }
}

impl From<LocalIoError> for BelowRootError {
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

    // EL-1: neither the diagnostic event nor the message a person is shown
    // names the folder the walk stopped at. It stays in the value, for the
    // caller that has somebody to answer with it.
    #[test]
    fn a_blocked_place_says_it_was_blocked_and_never_where_it_stopped() {
        let refused = BelowRootError::Blocked {
            stopped_at: PathBuf::from("/home/someone/albums/link"),
        };

        assert_eq!(refused.redacted(), "BelowRoot::Blocked");
        assert!(refused.to_string().contains("mapped root"));
        assert!(
            !refused.to_string().contains("someone"),
            "the folder stays in the value and out of the message",
        );
    }

    #[test]
    fn a_refused_call_says_what_it_was_doing_and_never_where() {
        let refused = BelowRootError::Io(LocalIoError::new(
            LocalOperation::Renaming,
            "/home/someone/albums/spring.jpg",
            io::Error::from(io::ErrorKind::PermissionDenied),
        ));

        assert_eq!(
            refused.redacted(),
            "BelowRoot::Io: Local::Io(operation=renamed, kind=PermissionDenied)",
        );
        assert!(
            !refused.redacted().contains("someone"),
            "no part of a local path may reach a diagnostic event",
        );
    }

    // This vocabulary says the step was taken below a mapped root, and the
    // capability's own refusal says which operation the disk refused and what
    // the operating system answered. A caller printing `{error:#}` reads each
    // of those once.
    #[test]
    fn a_refused_call_reaches_a_caller_as_one_sentence_per_layer() {
        let refused = BelowRootError::Io(LocalIoError::new(
            LocalOperation::Renaming,
            "/home/someone/albums/spring.jpg",
            io::Error::from(io::ErrorKind::PermissionDenied),
        ));

        assert_eq!(
            chain(&refused),
            vec![
                "a step below the mapped root could not be taken on this device".to_owned(),
                "a local file or folder could not be renamed".to_owned(),
                "permission denied".to_owned(),
            ],
        );
    }
}
