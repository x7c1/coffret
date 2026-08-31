use super::{Noted, SyncStatus};
use crate::reported::Reported;

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
    pub noted: Vec<Noted>,
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
            status: SyncStatus::Syncing,
            added: 0,
            noted: Vec::new(),
            stopped: None,
        }
    }
}
