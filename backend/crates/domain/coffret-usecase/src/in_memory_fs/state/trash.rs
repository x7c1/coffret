use std::io;
use std::path::{Path, PathBuf};

use crate::descent_error::DescentError;
use crate::device_state::RootMarkerId;
use crate::in_memory_fs::state::State;
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;
use crate::mapped_relative_location::MappedRelativeLocation;

impl State {
    /// Moves one regular file below a vouched root into the fake's trash
    /// (spec: EP-13, EP-15).
    ///
    /// The same order the real one keeps: the scripted refusal first, so a
    /// refused move leaves the file exactly where it was; then the root and its
    /// marker; then every folder on the way down, each of which has to be a
    /// folder; and last the file itself, which has to be a file.
    pub(in crate::in_memory_fs) fn move_to_trash(
        &mut self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        relative: &MappedRelativeLocation,
    ) -> Result<(), DescentError> {
        let path = root.join(relative.to_path_buf());
        self.attempt(LocalOperation::MovingToTrash, &path)
            .map_err(DescentError::Io)?;
        if !self.is_dir(root) {
            return Err(DescentError::Io(LocalIoError::new(
                LocalOperation::Stating,
                root,
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "the mapped root is not a folder here",
                ),
            )));
        }
        self.vouch(root, expected)?;

        let mut folder = root.to_path_buf();
        let components: Vec<_> = relative.components().collect();
        let (_, folders) = components
            .split_last()
            .expect("a file below a mapped root names at least itself");
        for step in folders {
            folder.push(step);
            if !self.is_dir(&folder) {
                return Err(DescentError::Blocked { stopped_at: folder });
            }
        }
        if self.files.remove(&path).is_none() {
            let kind = if self.holds(&path) {
                io::ErrorKind::Other
            } else {
                io::ErrorKind::NotFound
            };
            return Err(DescentError::Io(LocalIoError::new(
                LocalOperation::MovingToTrash,
                &path,
                io::Error::new(kind, "no regular file is at this path"),
            )));
        }
        self.moved_to_trash.push(path);
        Ok(())
    }

    /// Everything the fake's trash has taken, in the order it took it.
    pub(in crate::in_memory_fs) fn moved_to_trash(&self) -> Vec<PathBuf> {
        self.moved_to_trash.clone()
    }
}
