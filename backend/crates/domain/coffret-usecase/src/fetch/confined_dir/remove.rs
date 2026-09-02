use rustix::fs::AtFlags;
use rustix::io::Errno;

use super::ConfinedDir;
use crate::fetch::descent_error::DescentError;
use crate::local_operation::LocalOperation;

impl ConfinedDir {
    /// Removes one of the folder's own files.
    ///
    /// One that is already gone is the outcome this wanted, so a cleanup racing
    /// the failure it is cleaning up after still succeeds.
    pub fn remove(&self, name: &str) -> Result<(), DescentError> {
        match rustix::fs::unlinkat(&self.directory, name, AtFlags::empty()) {
            Ok(()) => Ok(()),
            Err(gone) if gone == Errno::NOENT => Ok(()),
            Err(cause) => Err(self.refused(name, LocalOperation::Removing, cause)),
        }
    }
}
