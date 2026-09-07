use std::io;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;

use crate::device_state::RootIdentity;
use crate::folder_entry::FolderEntry;
use crate::in_memory_fs::in_memory_source_reader::InMemorySourceReader;
use crate::in_memory_fs::state::lock;
use crate::in_memory_fs::InMemoryFs;
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;
use crate::mapped_roots::MappedRoots;
use crate::root_probe::RootProbe;
use crate::source_reader::SourceReader;

/// What the fake answers for a root no case gave an identity of its own.
///
/// A value rather than `None`, because "this platform can say nothing" is a
/// different case from "this root stands on some filesystem", and every case
/// about a root that moved needs the second. It is deliberately not spelled the
/// way any real platform spells one, so a mapping a case recorded with another
/// device's spelling mismatches it — which is exactly what an unmounted disk
/// looks like to the comparison (spec: EP-12).
const IN_MEMORY_IDENTITY: &str = "in-memory:0";

#[async_trait]
impl MappedRoots for InMemoryFs {
    async fn probe_root(&self, root: &Path) -> Result<Option<RootProbe>, LocalIoError> {
        let mut state = lock(&self.state);
        state.attempt(LocalOperation::Stating, root)?;
        if !state.holds(root) {
            return Ok(None);
        }
        let identity = state
            .root_identity(root)
            .cloned()
            .unwrap_or_else(|| RootIdentity::new(IN_MEMORY_IDENTITY));
        Ok(Some(RootProbe {
            identity: Some(identity),
        }))
    }

    async fn list_folder(&self, dir: &Path) -> Result<Option<Vec<FolderEntry>>, LocalIoError> {
        let mut state = lock(&self.state);
        state.attempt(LocalOperation::Listing, dir)?;
        if state.is_dir(dir) {
            return Ok(Some(state.list(dir)));
        }
        if state.holds(dir) {
            // A real listing of something that is not a directory is refused
            // rather than answered with nothing, and the fake refuses it too:
            // absence is the only thing `None` may stand for.
            return Err(LocalIoError::new(
                LocalOperation::Listing,
                dir,
                io::Error::other("what is at this path is not a folder"),
            ));
        }
        Ok(None)
    }

    async fn open_source(&self, path: &Path) -> Result<Box<dyn SourceReader>, LocalIoError> {
        let mut state = lock(&self.state);
        state.attempt(LocalOperation::Reading, path)?;
        let content = state.content(path).ok_or_else(|| {
            LocalIoError::new(
                LocalOperation::Reading,
                path,
                io::Error::new(io::ErrorKind::NotFound, "no file is at this path"),
            )
        })?;
        drop(state);
        Ok(Box::new(InMemorySourceReader::new(
            Arc::clone(&self.state),
            path.to_path_buf(),
            content,
        )))
    }
}
