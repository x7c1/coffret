use coffret_model::{EntryLocation, EntryPath};

use crate::device_state::local_entry_state::LocalEntryState;
use crate::device_state::local_observation::LocalObservation;
use crate::entry_ref::EntryRef;

/// One row of what this device has, or had, on disk.
///
/// The row outlives the Entry it was made for. A path that leaves the Library —
/// because another device removed the Container holding it — keeps its row
/// here, which is what lets the device still answer "I have a file at a path
/// the Library no longer lists" instead of quietly leaving it on disk unnoticed
/// (spec: EP-10, CK-7).
///
/// It is a record of one Entry and of no other:
/// [`observation.entry`](LocalObservation::entry) names the Entry the file is a
/// copy of. Wherever the catalog comes to place a different Entry at the path
/// the file stands at, the Index forgets the row rather than let it claim that
/// Entry, or the row waits to follow its own Entry elsewhere (spec: EP-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalEntry {
    /// What the device last saw of the file, and which Entry it is a copy of.
    pub observation: LocalObservation,
    /// Whether the file is there now.
    pub state: LocalEntryState,
    /// Where the Entry this file is a copy of stands now, when a rename has
    /// changed its current Entry Path since the file was placed and the file
    /// has not been relocated to follow it; `None` otherwise.
    ///
    /// The row stays keyed by the path the file stands at, because that is
    /// where the file is. But it is no longer the Entry at that path: the scan
    /// neither carries it in as a new file nor reports it as deleted or
    /// departed, and nothing selects it for `update` or `freeze`, until a sync
    /// or a fetch relocates the file to follow (spec: FM-15, EP-10).
    pub relocate_to: Option<EntryPath>,
}

impl LocalEntry {
    /// Whether this row's file waits to follow a rename of its Entry
    /// elsewhere.
    pub const fn awaits_relocation(&self) -> bool {
        self.relocate_to.is_some()
    }

    /// Whether this row is this device's record of the Entry at `location`,
    /// standing at that Entry's own path: the same Entry, and no relocation
    /// pending (spec: EP-10).
    ///
    /// What every reader asks before it takes the row as a copy of the Entry
    /// it is looking at, so that a row about another Entry is never read as
    /// this one's — whatever the path says.
    pub fn is_copy_of(&self, location: &EntryLocation) -> bool {
        !self.awaits_relocation() && self.observation.entry == EntryRef::of(location)
    }
}
