use std::path::Path;

use crate::descent_error::DescentError;
use crate::device_state::{RootIdentity, RootMarkerId};
use crate::in_memory_fs::state::State;
use crate::refused_root::RootRefused;
use crate::root_marker;

impl State {
    /// Records what [`probe_root`](crate::MappedRoots::probe_root) answers for
    /// one path.
    pub(in crate::in_memory_fs) fn set_root_identity(
        &mut self,
        path: &Path,
        identity: RootIdentity,
    ) {
        self.identities.insert(path.to_path_buf(), identity);
    }

    /// What the fake says the filesystem under `path` is.
    pub(in crate::in_memory_fs) fn root_identity(&self, path: &Path) -> Option<&RootIdentity> {
        self.identities.get(path)
    }

    /// Whether the root at `path` is the root a mapping expecting `expected` was
    /// recorded against (spec: EP-13).
    ///
    /// The same sequence the real descent makes, in the terms the fake has for
    /// it: the management area has to be a folder, the marker a file, its content
    /// a marker, and its identity the one the mapping recorded. A planted "other"
    /// standing at either name is the shape a symbolic link has here, and it
    /// answers the same way one does over there — the name is not the required
    /// kind.
    ///
    /// Read-only, and deliberately so: nothing about a marker is ever created or
    /// repaired by a placement, in the fake as on a disk.
    pub(in crate::in_memory_fs) fn vouch(
        &self,
        path: &Path,
        expected: Option<&RootMarkerId>,
    ) -> Result<(), DescentError> {
        let refused = |reason| {
            Err(DescentError::Refused {
                root: path.to_path_buf(),
                reason,
            })
        };

        // Asked before the folder is looked at, because a mapping with no
        // identity has nothing to hold a marker against however sound the marker
        // is.
        let Some(expected) = expected else {
            return refused(RootRefused::NoExpectedIdentity);
        };

        let area = path.join(root_marker::MANAGEMENT_AREA);
        if !self.holds(&area) {
            return refused(RootRefused::ManagementAreaMissing);
        }
        if !self.is_dir(&area) {
            return refused(RootRefused::ManagementAreaNotADirectory);
        }

        let marker = area.join(root_marker::MARKER_FILE);
        let Some(content) = self.content(&marker) else {
            return if self.holds(&marker) {
                // A folder, or the fake's stand-in for a link: something is at
                // the name and it is not the file the rule is about.
                refused(RootRefused::MarkerNotARegularFile)
            } else {
                refused(RootRefused::MarkerMissing)
            };
        };

        // Bounded the way the real read is bounded: the parse sees one byte past
        // the cap, which is the least that shows the content running on, and
        // nothing beyond it settles anything (spec: EP-13). The fake holds the
        // bytes already, so what this models is the reading's bound rather than
        // its cost.
        let read = content.len().min(root_marker::MAX_LEN + 1);
        match root_marker::parse(&content[..read]) {
            Err(cause) => refused(RootRefused::MarkerMalformed { cause }),
            Ok(found) if found != *expected => refused(RootRefused::MarkerMismatch),
            Ok(_) => Ok(()),
        }
    }
}
