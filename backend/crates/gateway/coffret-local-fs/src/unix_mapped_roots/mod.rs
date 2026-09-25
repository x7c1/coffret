mod list_folder;
mod open_components;
mod probe_root;
mod source_reader;

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

    async fn source_reader(
        &self,
        root: &Path,
        relative: &MappedRelativeLocation,
    ) -> Result<Box<dyn SourceReader>, LocalIoError> {
        source_reader::source_reader(root, relative).await
    }
}
