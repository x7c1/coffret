use std::io;
use std::path::Path;

use async_trait::async_trait;
use coffret_local_fs::UnixFs;
use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::{
    DescentError, LocalIoError, LocalOperation, LocalTrash, MappedRelativeLocation,
};

/// A trash that takes a discarded file by removing it from the mapped folder,
/// and never asks the desktop's own trash for anything.
///
/// The served device and the other one discard through this rather than
/// through [`UnixFs`]: that one asks the trash of the desktop the suite runs
/// on, which would put the fixtures' files into the developer's own trash on
/// every run, and would fail a case wherever that trash refuses them — a
/// sandbox, or a runner with no trash to speak of. What a case is about is
/// the route, and what it asserts is that the file left the mapped folder,
/// which removing it answers just as well.
///
/// The descent and the vouching a real discard makes before it asks the trash
/// (spec: EP-8, EP-13) are not repeated here; `coffret-local-fs` tests those
/// against the real one. The removal of emptied folders and of stale
/// scratches never reaches the desktop's trash, so those are the real ones.
#[derive(Debug, Default)]
pub struct RemovingTrash {
    disk: UnixFs,
}

#[async_trait]
impl LocalTrash for RemovingTrash {
    async fn discard(
        &self,
        root: &Path,
        _expected: Option<&RootMarkerId>,
        relative: &MappedRelativeLocation,
    ) -> Result<(), DescentError> {
        let file = root.join(relative.to_path_buf());
        let refused = |cause: io::Error| {
            DescentError::Io(LocalIoError::new(LocalOperation::Discarding, &file, cause))
        };
        // Stated without following a link, as the real one states it: a
        // symbolic link at the name is not the file, and is left alone.
        let standing = std::fs::symlink_metadata(&file).map_err(refused)?;
        if !standing.file_type().is_file() {
            return Err(refused(io::Error::other(
                "what stands at this path is not a regular file",
            )));
        }
        std::fs::remove_file(&file).map_err(refused)
    }

    async fn remove_emptied_folder(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        relative: &MappedRelativeLocation,
    ) -> Result<bool, DescentError> {
        self.disk
            .remove_emptied_folder(root, expected, relative)
            .await
    }

    async fn remove_scratch(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        relative: &MappedRelativeLocation,
    ) -> Result<bool, DescentError> {
        self.disk.remove_scratch(root, expected, relative).await
    }
}
