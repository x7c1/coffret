use std::path::Path;

use crate::device_state::RootMarkerId;
use crate::mapped_relative_location::MappedRelativeLocation;

/// One name below a mapped root, with the identity the root is held against:
/// one side of a [`Relocations`](crate::Relocations) call.
///
/// The root and its expected identity travel with the name because a
/// relocation may leave one mapped root for another that stands on the same
/// filesystem, and each root vouches for itself before anything is written
/// into it (spec: EP-10, EP-13).
#[derive(Debug, Clone, Copy)]
pub struct MappedName<'a> {
    /// The mapped root the name is below.
    pub root: &'a Path,
    /// The identity the mapping records for that root, or `None` where it
    /// records none — which a write into the root refuses (spec: EP-13).
    pub expected: Option<&'a RootMarkerId>,
    /// The name below the root.
    pub relative: &'a MappedRelativeLocation,
}

impl MappedName<'_> {
    /// Whether `other` is below the same mapped root.
    pub fn same_root(&self, other: &MappedName<'_>) -> bool {
        self.root == other.root
    }
}
