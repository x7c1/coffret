use crate::device_state::LocalObservation;
use crate::freeze::not_frozen::NotFrozen;
use crate::freeze::selected::Selected;

/// What one scan of the folder concluded.
///
/// No plaintext travels in it — a selection names a file and the spool step is
/// what opens it again — so a survey of a folder of several hundred gigabytes
/// weighs what its Entry Paths and hashes weigh.
pub(super) struct Survey {
    /// The files to pack, in Entry Path order (spec: PK-3).
    pub(super) selected: Vec<Selected>,
    /// How many Entries a Pack already holds and the local file still matches.
    pub(super) packed_already: usize,
    /// What the scan surfaces and does not act on (spec: PK-14).
    pub(super) surfaced: Vec<NotFrozen>,
    /// What to write down about files that turned out unchanged after being
    /// read: they were touched, so the length and modification time this device
    /// last saw are stale even though the content is not (spec: EP-10).
    pub(super) refreshed: Vec<LocalObservation>,
}
