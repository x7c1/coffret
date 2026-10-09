use coffret_model::EntryPath;

use crate::commit::DegradedKeyring;
use crate::fetch::surfaced::Surfaced;
use crate::fetch::unheld_parcel::UnheldParcel;

/// What one run of [`fetch_entry`](super::fetch_entry) came to.
///
/// Four answers rather than a count, because a run of one Entry has exactly
/// four things it can have done, and each is a different thing for a caller to
/// do next. Placing it is the answer a reader waited for. Finding it already
/// materialized is the same availability at no cost. Declining it is the finding
/// EP-11 will not let a run keep to itself — the file is not there, the run
/// succeeded, and the reason has to travel with the answer. And stopping is
/// what a caller that cancelled hears: the run asked for no further parcel
/// (spec: PK-21).
///
/// There is no "the Container is now fetched" among them, and that is the point
/// of PK-16: the unit read is the parcel, so what is on the device afterwards is
/// the parcels read and the Entries they covered — never a claim about the rest
/// of the Container (spec: PK-22).
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
    /// The caller cancelled before a parcel the Entry needed was asked for, so
    /// it is not placed (spec: PK-21).
    ///
    /// Only a caller that handed the run a
    /// [`Cancellation`](super::Cancellation) hears this. The parcels read
    /// before it are held, and whatever they covered may already be placed —
    /// [`EntryFetchOutcome::alongside`] says which.
    Cancelled,
}

/// What one run of [`fetch_entry`](super::fetch_entry) came to, and what it
/// noticed about the Library on the way.
///
/// The verdict is [`EntryFetch`]'s four answers. Beside it are the things a run
/// of one Entry can notice or do that are not about that Entry: the committed
/// Keyring set it read the envelope from, where the read had to step over a
/// position to reach a valid replica (spec: KL-5, KL-15) — a reader that only
/// ever opens files would otherwise never hear of it, since a fetch writes
/// nothing and repairs nothing; the Entries the parcels it read placed besides
/// (spec: PK-16); and the kept parcels it found were not held after all
/// (spec: PK-21).
#[derive(Debug, Clone)]
pub struct EntryFetchOutcome {
    /// What became of the Entry.
    pub fetch: EntryFetch,
    /// The finding about the committed Keyring, where the run read one and had
    /// to step over a position of it. `None` where it read no Keyring at all —
    /// the Entry was already here, or declined before the envelope mattered.
    pub degraded: Option<DegradedKeyring>,
    /// The other Entries placed out of the parcels the run read or held, in
    /// stream order (spec: PK-16).
    ///
    /// A parcel is read whole, so the Entries inside it come with the one asked
    /// for, and those this device would place anyway are placed. A caller
    /// counting what is on disk counts these too.
    pub alongside: Vec<EntryPath>,
    /// The parcels this device's record said it held and did not — gone, or no
    /// longer authenticating — each read again from Storage (spec: PK-21).
    pub unheld: Vec<UnheldParcel>,
}

impl EntryFetchOutcome {
    /// A verdict reached without reading the committed Keyring.
    pub fn of(fetch: EntryFetch) -> Self {
        Self {
            fetch,
            degraded: None,
            alongside: Vec::new(),
            unheld: Vec::new(),
        }
    }
}
