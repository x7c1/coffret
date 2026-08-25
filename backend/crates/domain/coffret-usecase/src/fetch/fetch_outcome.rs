use coffret_model::{ContainerId, EntryPath};

use crate::fetch::surfaced::Surfaced;

/// What one fetch run placed, and what it did not.
///
/// The second half is the one that matters most, and it is why there is no
/// single "how many files did you fetch" number. A folder is a copy of its part
/// of the Library only when [`surfaced`](Self::surfaced) is empty: every entry in
/// it is a path the run declined, and a caller that reads only
/// [`fetched`](Self::fetched) would believe the folder complete when it is not
/// (spec: EP-11).
#[derive(Debug)]
pub struct FetchOutcome {
    /// The Entries this run wrote into the mapped folders, in Entry Path order.
    ///
    /// Each of them is now this device's own materialization, so the next scan
    /// sees a clean match and the file is in the sync flow's scope from here on
    /// (spec: EP-10).
    pub fetched: Vec<EntryPath>,
    /// The Containers the run fetched, once each, in Container ID order.
    ///
    /// The fetch unit is a whole Container however many of its Entries were
    /// wanted (spec: PK-16), so this is shorter than
    /// [`fetched`](Self::fetched) wherever a Pack held several of them.
    pub containers: Vec<ContainerId>,
    /// How many selected Entries were already materialized here.
    ///
    /// The device's own record matches the file on disk, so the file is the
    /// Entry and there is nothing to fetch (spec: EP-10, EP-11).
    pub skipped: usize,
    /// Every Entry the run selected and did not place, with the reason
    /// (spec: EP-11).
    pub surfaced: Vec<Surfaced>,
    /// The Containers the committed Keyring records no key for, in Container ID
    /// order (spec: KL-7).
    ///
    /// Reported at the Container level as well as per Entry, because that is the
    /// level the loss is at: one marker locks every Entry the Container holds,
    /// and healing it is one act rather than one per file (spec: KL-17, RV-7).
    pub locked: Vec<ContainerId>,
}
