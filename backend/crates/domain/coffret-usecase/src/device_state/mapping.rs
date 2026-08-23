use std::path::PathBuf;

use coffret_model::EntryPath;

/// Where one part of the Library lives on this device's disks.
///
/// A device maps each local root either to the Library root or to a top-level
/// Entry Path component, at most one of each, and when both are present the
/// top-level mapping represents that subtree while the root mapping represents
/// the remainder (spec: EP-9). The mappings are device state and are never
/// uploaded, so another device may arrange the same Library differently
/// (spec: CK-7).
///
/// A mapping translates Entry Paths into local paths and asserts nothing about
/// what is on disk: a device that maps `albums/` but has fetched only part of
/// it holds a partial subtree, and the rest does not count as deleted
/// (spec: EP-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    /// The top-level component this local root stands for, or `None` for the
    /// Library root itself.
    pub prefix: Option<EntryPath>,
    /// The folder on this device that the prefix is rooted at.
    pub local_root: PathBuf,
}
