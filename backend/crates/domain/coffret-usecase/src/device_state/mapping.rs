use std::path::PathBuf;

use coffret_model::EntryPath;

use crate::device_state::root_identity::RootIdentity;

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
///
/// [`root_identity`](Self::root_identity) is the one thing a mapping *does*
/// assert, and it is still not about what the root holds: it is which filesystem
/// the root stood on when a scan last looked. That is what lets an empty root be
/// told apart from a folder that was emptied — an unmounted mount point is an
/// ordinary empty directory, and the filesystem under it is not the one the
/// mounted disk carried, so nothing under such a root is read as evidence about
/// the Library (spec: EP-12). A mapping recorded afresh carries no identity, and
/// the next scan stamps whatever is there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    /// The top-level component this local root stands for, or `None` for the
    /// Library root itself.
    pub prefix: Option<EntryPath>,
    /// The folder on this device that the prefix is rooted at.
    pub local_root: PathBuf,
    /// What the filesystem under `local_root` was when a scan last saw it, or
    /// `None` where no scan has yet seen it (spec: EP-12).
    pub root_identity: Option<RootIdentity>,
}
