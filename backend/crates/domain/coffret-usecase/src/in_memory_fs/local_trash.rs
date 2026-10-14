use std::path::Path;

use async_trait::async_trait;

use crate::descent_error::DescentError;
use crate::device_state::RootMarkerId;
use crate::in_memory_fs::state::lock;
use crate::in_memory_fs::InMemoryFs;
use crate::local_trash::LocalTrash;
use crate::mapped_relative_location::MappedRelativeLocation;

#[async_trait]
impl LocalTrash for InMemoryFs {
    async fn move_to_trash(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        relative: &MappedRelativeLocation,
    ) -> Result<(), DescentError> {
        lock(&self.state).move_to_trash(root, expected, relative)
    }

    async fn remove_emptied_folder(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        relative: &MappedRelativeLocation,
    ) -> Result<bool, DescentError> {
        lock(&self.state).remove_emptied_folder(root, expected, relative)
    }
}
