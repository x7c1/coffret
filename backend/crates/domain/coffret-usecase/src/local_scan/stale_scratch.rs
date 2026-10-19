//! A scratch of coffret's own that a walk met and nothing holds: what a local
//! writer killed before its rename or its removal left behind (spec: OC-8,
//! EP-11).

use std::path::PathBuf;

use crate::device_state::RootMarkerId;
use crate::mapped_relative_location::MappedRelativeLocation;

/// Where one stale scratch stands, and what its root is held to.
///
/// The removal is a write into the mapped folder, so it carries what any write
/// there carries: the root, the identity its mapping records for it, and the
/// location below it as the folder spelled it (spec: EP-8, EP-13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StaleScratch {
    /// The mapped root it was found under.
    pub(crate) root: PathBuf,
    /// The identity that root's mapping records: only a root with one is ever
    /// written into, by a writer or by this removal (spec: EP-13).
    pub(crate) expected: RootMarkerId,
    /// Where it stands below the root.
    pub(crate) relative: MappedRelativeLocation,
}
