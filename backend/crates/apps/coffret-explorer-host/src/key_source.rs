use std::io;

/// Where the host reads the key the server admits callers by (spec: LA-3).
///
/// A trait rather than a path read in place, so that a case can count how
/// often the host reads it: never, for a request that did not pass every fence
/// (spec: LA-14). [`KeyFile`](crate::KeyFile) is what the host runs on.
pub trait KeySource: Send + Sync {
    /// The key as it stands now, trimmed of the line end it was written with.
    ///
    /// An error is handed back as it arrived; what it is answered with is the
    /// host's to decide, and it never renders the error's message, which may
    /// name the file (spec: EL-3).
    fn read(&self) -> io::Result<String>;
}
