use std::path::Path;

use crate::device_state::RootIdentity;
use crate::in_memory_fs::state::State;

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
}
