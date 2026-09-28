use crate::commit::DegradedKeyring;
use crate::fetch::surfaced::Surfaced;

/// What one run of [`fetch_entry`](super::fetch_entry) came to.
///
/// Three answers rather than a count, because a run of one Entry has exactly
/// three things it can have done, and each is a different thing for a caller to
/// do next. Placing it is the answer a reader waited for. Finding it already
/// materialized is the same availability at no cost. Declining it is the finding
/// EP-11 will not let a run keep to itself — the file is not there, the run
/// succeeded, and the reason has to travel with the answer.
///
/// There is no "the Container is now fetched" among them, and that is the point
/// of PK-16: a range read is a step inside fetching the containing Container,
/// so the rest of that Container is exactly as unfetched afterwards as it was
/// before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryFetch {
    /// The Entry's file is on disk at its mapped path, verified, stamped with
    /// the Entry's own modification time, and recorded as this device's own
    /// materialization (spec: EP-10, EP-11).
    Placed,
    /// This device already had the file, and its own materialization record
    /// still matches what is on disk — so the file *is* the Entry and there was
    /// nothing to fetch (spec: EP-10, EP-11).
    AlreadyPresent,
    /// The run declined to place the Entry, with the reason (spec: EP-11).
    Surfaced(Surfaced),
}

/// What one run of [`fetch_entry`](super::fetch_entry) came to, and what it
/// noticed about the Library on the way.
///
/// The verdict is [`EntryFetch`]'s three answers. Beside it is the one thing a
/// run of one Entry can notice that is not about that Entry: the committed
/// Keyring set it read the envelope from, where the read had to step over a
/// position to reach a valid replica (spec: KL-5, KL-15). A reader that only
/// ever opens files would otherwise never hear of it, since a fetch writes
/// nothing and repairs nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryFetchOutcome {
    /// What became of the Entry.
    pub fetch: EntryFetch,
    /// The finding about the committed Keyring, where the run read one and had
    /// to step over a position of it. `None` where it read no Keyring at all —
    /// the Entry was already here, or declined before the envelope mattered.
    pub degraded: Option<DegradedKeyring>,
}

impl EntryFetchOutcome {
    /// A verdict reached without reading the committed Keyring.
    pub fn of(fetch: EntryFetch) -> Self {
        Self {
            fetch,
            degraded: None,
        }
    }
}
