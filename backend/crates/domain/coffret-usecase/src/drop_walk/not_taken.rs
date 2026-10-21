use super::NotTakenReason;

/// Something the walk met and did not take, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotTaken {
    /// Where it stands relative to the folder the dropped item stood in, as
    /// [`DroppedSource::path`](super::DroppedSource::path) is spelled; for a
    /// name that is not text, what of it is.
    pub path: String,
    /// Why it was not taken.
    pub reason: NotTakenReason,
}
