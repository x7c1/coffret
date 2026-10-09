use crate::fetch::entry_fetch::EntryFetchOutcome;

/// Whoever asked for an Entry hearing that it is on disk, before the parcels
/// it came out of have finished arriving.
///
/// A parcel is read whole whatever Entry it was read for (spec: PK-16), so the
/// page a reader asked for is placed — verified, renamed, marked present
/// (spec: EP-11) — while the rest of its parcel is still on the way, and a
/// parcel is tens of megabytes. The Entry is released as soon as the chunks
/// covering it have arrived, and this is where it is released to: a caller
/// with a reader waiting answers it from here and lets the run go on reading
/// the parcel to its end, keeping it, and placing what else it covers
/// (spec: PK-21).
///
/// Called at most once per run, and only where the run placed the Entry out of
/// parcels: an Entry already here, one declined, or one the caller cancelled
/// before it arrived is answered by the run's own end and never by this. What
/// it is handed is the outcome as it stands at that moment — the Entry
/// [`Placed`](super::EntryFetch::Placed), the Keyring finding the run read on
/// the way, the Entries placed before it, and the kept parcels found not held
/// before it. The run's own answer is the whole account, and a caller that
/// returns from here owes the rest of the run a place to finish.
///
/// A trait over a plain callback for the reason
/// [`Cancellation`](super::Cancellation) is one: the caller knows what to do
/// with the news, and the fetch has no business knowing.
pub trait Publication: Send + Sync {
    /// The Entry asked for has just been published.
    fn published(&self, outcome: &EntryFetchOutcome);
}

impl<F> Publication for F
where
    F: Fn(&EntryFetchOutcome) + Send + Sync,
{
    fn published(&self, outcome: &EntryFetchOutcome) {
        self(outcome)
    }
}
