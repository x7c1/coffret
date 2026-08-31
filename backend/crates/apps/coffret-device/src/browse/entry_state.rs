/// Whether the file for one Entry is on this device right now.
///
/// Two states and not three. The Library holds an Entry either way — that is
/// what the catalog is for, and it catalogs the whole Library rather than the
/// part this device keeps on disk (spec: CK-7) — so what this adds is the one
/// thing only this device knows: whether it materialized the Entry and still
/// has it (spec: EP-10).
///
/// A mapping is not among the evidence. A mapping translates Entry Paths into
/// local paths and asserts nothing about what is on disk, so a device that maps
/// a folder and has fetched none of it holds nothing there (spec: EP-9, EP-10).
/// A present materialization record is the only claim.
///
/// What a reader shows while a fetch is running, and what it shows when one
/// failed, are the reader's own states over a request it made. They are not
/// here because the catalog has no such rows: nothing on this device changes
/// between asking for an Entry and getting it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryState {
    /// This device has the file, having uploaded or fetched it into place
    /// (spec: EP-10).
    Present,
    /// This device does not have the file: it never materialized the Entry, or
    /// it did and has witnessed the file go (spec: EP-10).
    Remote,
}
