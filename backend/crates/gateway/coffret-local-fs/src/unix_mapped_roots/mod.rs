mod list_folder;
mod open_components;
mod open_source;
mod probe_root;

use std::path::Path;

use async_trait::async_trait;
use coffret_usecase::{
    FolderEntry, LocalIoError, MappedRelativeLocation, MappedRoots, RootProbe, SourceReader,
};

use crate::UnixFs;

#[async_trait]
impl MappedRoots for UnixFs {
    async fn probe_root(&self, root: &Path) -> Result<Option<RootProbe>, LocalIoError> {
        probe_root::probe_root(root).await
    }

    async fn list_folder(
        &self,
        root: &Path,
        relative: Option<&MappedRelativeLocation>,
    ) -> Result<Option<Vec<FolderEntry>>, LocalIoError> {
        list_folder::list_folder(root, relative).await
    }

    async fn open_source(
        &self,
        root: &Path,
        relative: &MappedRelativeLocation,
    ) -> Result<Box<dyn SourceReader>, LocalIoError> {
        open_source::open_source(root, relative).await
    }
}
