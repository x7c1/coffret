use std::path::Path;

use async_trait::async_trait;

use crate::device_state::RootMarkerId;
use crate::in_memory_fs::state::lock;
use crate::in_memory_fs::InMemoryFs;
use crate::mapped_relative_location::MappedRelativeLocation;
use crate::relocate_error::RelocateError;
use crate::relocations::Relocations;

#[async_trait]
impl Relocations for InMemoryFs {
    async fn relocate(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        from: &MappedRelativeLocation,
        to: &MappedRelativeLocation,
    ) -> Result<(), RelocateError> {
        lock(&self.state).relocate(root, expected, from, to)
    }

    async fn exchange(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        one: &MappedRelativeLocation,
        other: &MappedRelativeLocation,
    ) -> Result<(), RelocateError> {
        lock(&self.state).exchange(root, expected, one, other)
    }
}
