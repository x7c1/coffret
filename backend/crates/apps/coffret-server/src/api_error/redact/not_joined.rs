use coffret_device::Redacted;
use tokio::task::JoinError;

/// A blocking task that ended without an answer, kept as the value the runtime
/// reported, under the identity the refusal is filed under.
///
/// An event is told the identity and whether the task panicked rather than
/// being cancelled, which is the half anything acts on. The panic's payload is
/// not told: it is whatever the task was in the middle of, and the task was
/// reading a local folder.
pub(crate) struct NotJoined(pub(crate) &'static str, pub(crate) JoinError);

impl Redacted for NotJoined {
    fn redacted(&self) -> String {
        format!("{}(panic={})", self.0, self.1.is_panic())
    }
}
