use crate::device_state::local_entry_state::LocalEntryState;
use crate::device_state::local_observation::LocalObservation;

/// One row of what this device has, or had, on disk.
///
/// The row outlives the Entry it was made for. A path that leaves the Library —
/// because another device removed the Container holding it — keeps its row
/// here, which is what lets the device still answer "I have a file at a path
/// the Library no longer lists" instead of quietly leaving it on disk unnoticed
/// (spec: EP-10, CK-7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalEntry {
    /// What the device last saw of the file.
    pub observation: LocalObservation,
    /// Whether the file is there now.
    pub state: LocalEntryState,
}
