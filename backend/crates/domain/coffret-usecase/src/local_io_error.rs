use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use coffret_model::Redacted;

use crate::local_operation::LocalOperation;

/// What one operation on this device's own disk failed with.
///
/// The vocabulary the capabilities over the local filesystem answer in —
/// [`Spool`](crate::Spool) is the first of them — so that a gateway outside
/// this crate, or any caller keeping files of its own on the same disk, can
/// report a refusal in the same three parts every flow here already reads:
/// what the run was doing, which file or directory it was doing it to, and
/// what the operating system said.
///
/// The path is in the value and not in the message, for the reason every error
/// carrying one keeps it there: a local path is one of the things that may
/// never reach a log line, and an error's message is the part most likely to be
/// logged verbatim (spec: EL-1, EL-3). [`Redacted`] is what a log line renders
/// instead.
///
/// The cause travels as the value the operating system produced rather than as
/// its message: its [`kind`](io::Error::kind) is what separates a full disk from
/// a path that is gone, and stringifying it on the way in would leave a reader
/// matching on prose to tell them apart. Nothing in the use-case layer branches
/// on that kind to interpret an answer — where absence is an ordinary outcome
/// the capability's own contract says so, and swallowing it is the gateway's
/// (spec: OC-6).
///
/// There is deliberately no `PartialEq`, for the reason the flows' error types
/// have none: a caller decides from the operation and the kind, never by
/// comparing two errors.
#[derive(Debug)]
pub struct LocalIoError {
    /// What the run was doing.
    pub operation: LocalOperation,
    /// The file or directory it was doing it to.
    pub path: PathBuf,
    /// What the operating system reported.
    pub cause: io::Error,
}

impl LocalIoError {
    /// A failed filesystem operation, as a gateway behind a capability — or a
    /// caller keeping files of its own on the same disk — reports it.
    pub fn new(operation: LocalOperation, path: impl Into<PathBuf>, cause: io::Error) -> Self {
        Self {
            operation,
            path: path.into(),
            cause,
        }
    }
}

impl fmt::Display for LocalIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The path stays out of the message and stays in the value: see the
        // type.
        write!(
            f,
            "a local file or folder could not be {}: {}",
            self.operation, self.cause
        )
    }
}

impl error::Error for LocalIoError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.cause)
    }
}

impl Redacted for LocalIoError {
    /// The operation and the kind of failure, and nothing about the file.
    ///
    /// The same two facts [`FetchError::Io`](crate::fetch::FetchError::Io)
    /// carries into a log line, because they are the same question asked of the
    /// same disk: which operation refused, and what sort of refusal it was.
    fn redacted(&self) -> String {
        format!(
            "Local::Io(operation={}, kind={:?})",
            self.operation,
            self.cause.kind()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // EL-1: the message a person is shown may say what happened, and the log
    // line may not say which file it happened to.
    #[test]
    fn a_refusal_says_what_it_was_doing_and_never_where() {
        let error = LocalIoError::new(
            LocalOperation::Flushing,
            "/home/someone/spool/a-container.spool",
            io::Error::from(io::ErrorKind::PermissionDenied),
        );

        assert_eq!(
            error.redacted(),
            "Local::Io(operation=flushed, kind=PermissionDenied)",
        );
        assert!(
            !error.redacted().contains("someone"),
            "no part of a local path may reach a log line",
        );
        assert!(error.to_string().contains("flushed"));
    }
}
