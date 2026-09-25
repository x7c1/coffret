use coffret_device::Step;

use crate::finding::Finding;
use crate::reported::Reported;

use super::SyncStatus;

/// What one sync has come to.
///
/// The server's own state and not the Library's: it is about work in flight on
/// this device, it is gone when the process is, and none of it is ever uploaded.
/// What the Library holds afterwards is the listing's to say, as it was before —
/// this says what a run of the flow did and what it left alone.
///
/// There is no folder here, unlike a fill's account of itself, and no count of
/// what a run set out to do. A sync covers the device's mappings entire and finds
/// what there is to find by walking them, so there is no total to state until the
/// walk is over; what is stated is what came of it.
#[derive(Clone, Debug)]
pub struct SyncActivity {
    /// Which run of the sync this is, counted from the start of this process.
    ///
    /// What a screen tells one run's account of itself from the next's: a line
    /// somebody has read and put away must not take the next run's line with it,
    /// and two runs that found the same thing are otherwise identical. Stamped
    /// on by [`Syncs`](super::Syncs) rather than carried here from the flow, so
    /// a fresh one is `0` until it is published.
    pub run: u64,
    /// Where the sync stands.
    pub status: SyncStatus,
    /// How many files the run carried into the Library — the files added and the
    /// ones that replaced an Entry alike (spec: CP-14).
    ///
    /// `0` until the run is over, and `0` afterwards for a run that found nothing
    /// to carry: the walk is what counts them, and a run of a folder nobody
    /// changed is meant to add nothing.
    pub added: usize,
    /// What the run found and did not act on (spec: PK-14, EP-10, EP-12).
    pub findings: Vec<Finding>,
    /// How far into the run the flow has got, and `None` before it has said and
    /// once it is over.
    ///
    /// The same [`Step`](coffret_device::Step) the command line draws its
    /// progress line from, reported by the same port and meaning the same thing:
    /// a phase of the flow and how many units of it are done. It is not this
    /// server's reading of how far along a run is — nothing here counts
    /// anything — which is why a browser and a terminal watching one Library
    /// cannot disagree about it.
    pub step: Option<Step>,
    /// The refusal that stopped the sync, where one did.
    ///
    /// One refusal and not one per file: what stops a sync is Storage being
    /// unreachable or this device's own catalog or disk refusing, and every file
    /// left in the walk would have met it identically.
    pub stopped: Option<Reported>,
}

impl SyncActivity {
    /// A sync that has been armed and has not walked anything yet.
    pub(super) fn starting() -> Self {
        Self {
            run: 0,
            status: SyncStatus::Syncing,
            added: 0,
            findings: Vec::new(),
            step: None,
            stopped: None,
        }
    }
}
