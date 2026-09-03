use std::io;

/// What the operating system refused, as an event may carry it.
///
/// The [`kind`](io::ErrorKind) alone. A failure this server observed itself
/// carries no path — the standard library does not put one in an `io::Error` —
/// but one that crossed a gateway boundary may have had a message folded into
/// it that names a file, and the kind is the half anything acts on anyway.
pub(crate) fn io_failure(cause: &io::Error) -> String {
    format!("Io(kind={:?})", cause.kind())
}
