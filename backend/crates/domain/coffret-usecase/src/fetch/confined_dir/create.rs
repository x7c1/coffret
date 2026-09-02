use rustix::fs::{Mode, OFlags};

use super::ConfinedDir;
use crate::fetch::descent_error::DescentError;
use crate::local_operation::LocalOperation;

impl ConfinedDir {
    /// Makes a new file in the folder and hands it back open for writing.
    ///
    /// `O_EXCL`, so a name that already exists is a refusal rather than a file
    /// two writers share, and `O_NOFOLLOW`, so a symbolic link that took the
    /// name first is refused instead of followed. Callers give it a scratch name
    /// ([`scratch`](crate::scratch)), which nothing else in the folder is using.
    pub fn create(&self, name: &str) -> Result<std::fs::File, DescentError> {
        // 0o666 before the umask, which is what `File::create` asks for: the
        // file becomes the Entry's on the rename, and a fetch does not decide
        // the permissions of a person's own folder.
        let opened = rustix::fs::openat(
            &self.directory,
            name,
            OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_bits_truncate(0o666),
        )
        .map_err(|cause| self.refused(name, LocalOperation::Creating, cause))?;
        Ok(std::fs::File::from(opened))
    }
}
