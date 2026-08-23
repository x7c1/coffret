/// Whether this device has the file for one Entry Path on disk.
///
/// The distinction is what EP-10 rests on. A device reports an Entry as deleted
/// locally only when it had materialized that Entry itself — uploaded it, or
/// fetched it into place — and the file is now gone. An Entry the device never
/// held is outside its scope rather than missing, whether or not a mapping
/// covers it, so it is never reported as modified, never selected for `update`
/// or `freeze`, and never proposed for removal.
///
/// Only a row that was once [`Present`](Self::Present) can therefore become
/// [`Absent`](Self::Absent): absence is a fact about a file this device put
/// there, not about every path in the Library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalEntryState {
    /// This device has the file on disk, having uploaded or fetched it there.
    Present,
    /// This device had the file and it is gone — the one shape a local deletion
    /// takes (spec: EP-10).
    Absent,
}
