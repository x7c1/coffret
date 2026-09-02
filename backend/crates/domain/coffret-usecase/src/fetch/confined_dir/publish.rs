use super::ConfinedDir;
use crate::fetch::descent_error::DescentError;
use crate::local_operation::LocalOperation;

impl ConfinedDir {
    /// Renames one of the folder's own files onto the file's final name.
    ///
    /// Both names are resolved against the open folder, so the rename lands
    /// where the descent arrived whatever has happened to the path above it
    /// since. A rename within one directory is atomic, which is what makes it
    /// the moment the file exists (spec: EP-11).
    pub fn publish(&self, from: &str) -> Result<(), DescentError> {
        rustix::fs::renameat(&self.directory, from, &self.directory, self.name.as_str())
            .map_err(|cause| self.refused(&self.name, LocalOperation::Renaming, cause))
    }
}
