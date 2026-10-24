use std::io;

use coffret_device::Redacted;

/// What the operating system refused, kept as the value it refused with.
///
/// An event is told the [`kind`](io::ErrorKind) alone. A failure this server
/// observed itself carries no path — the standard library does not put one in
/// an `io::Error` — but one that crossed a gateway boundary may have had a
/// message folded into it that names a file, and the kind is the half anything
/// acts on anyway.
pub(crate) struct IoFailure(pub(crate) io::Error);

impl Redacted for IoFailure {
    fn redacted(&self) -> String {
        format!("Io(kind={:?})", self.0.kind())
    }
}
