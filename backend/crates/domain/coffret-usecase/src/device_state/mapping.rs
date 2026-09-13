use std::path::PathBuf;

use coffret_model::EntryPath;

use crate::device_state::root_identity::RootIdentity;
use crate::device_state::root_marker_id::RootMarkerId;

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
/// What a mapping *does* assert is two things about the root, and they answer
/// different questions.
///
/// [`root_identity`](Self::root_identity) answers *is the root there to be read
/// from* — it is which filesystem the root stood on when a scan last looked,
/// which is what lets an empty root be told apart from a folder that was
/// emptied: an unmounted mount point is an ordinary empty directory, and the
/// filesystem under it is not the one the mounted disk carried, so nothing under
/// such a root is read as evidence about the Library (spec: EP-12). It is
/// observed rather than chosen, and a scan re-stamps it whenever a root holding
/// files stands on a different one.
///
/// [`expected_root_id`](Self::expected_root_id) answers *is this the root that
/// was registered* — it is the identifier this device wrote into, or adopted
/// from, the marker file standing in the root when the mapping was recorded, and
/// what a later placement compares that marker against before it writes anything
/// (spec: EP-13). It is chosen rather than observed, drawn once and never
/// re-stamped by ordinary operation: only recording the mapping sets it.
///
/// So one says the folder is reachable and the other says the folder is the
/// right one, and neither says what is in it.
///
/// A mapping recorded afresh carries no filesystem identity, and the next scan
/// stamps whatever is there.
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
    /// The identity the marker in `local_root` is expected to carry, or `None`
    /// where the mapping records none (spec: EP-13).
    ///
    /// `None` is not a mapping that skips the check: a device placing anything
    /// through a mapping with no expected identity refuses, because there is
    /// nothing for the marker it finds to agree with.
    pub expected_root_id: Option<RootMarkerId>,
}

impl Mapping {
    /// A mapping as it is first recorded: a prefix, the folder it is rooted at,
    /// and no identity, because none has been observed yet — the next scan
    /// stamps whichever filesystem it finds the root standing on (spec: EP-12).
    ///
    /// This is also what re-confirms a root a run reported unavailable:
    /// [`set_mapping`](crate::Index::set_mapping) stores the mapping as given,
    /// so recording one afresh clears the identity held for that prefix.
    pub fn new(prefix: Option<EntryPath>, local_root: PathBuf) -> Self {
        Self {
            prefix,
            local_root,
            root_identity: None,
            expected_root_id: None,
        }
    }

    /// The same mapping carrying the identity a scan observed: for a row read
    /// back, and for a scan that re-stamps a root it has just looked at
    /// (spec: EP-12).
    pub fn stamped(self, root_identity: RootIdentity) -> Self {
        Self {
            root_identity: Some(root_identity),
            ..self
        }
    }

    /// The same mapping carrying the identity it expects the root's marker to
    /// hold: for a row read back, and for the registration that has just written
    /// or adopted that marker (spec: EP-13).
    pub fn expecting(self, expected_root_id: RootMarkerId) -> Self {
        Self {
            expected_root_id: Some(expected_root_id),
            ..self
        }
    }
}
