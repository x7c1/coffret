use async_trait::async_trait;

use crate::in_memory_fs::state::lock;
use crate::in_memory_fs::InMemoryFs;
use crate::mapped_name::MappedName;
use crate::relocate_error::RelocateError;
use crate::relocations::Relocations;

#[async_trait]
impl Relocations for InMemoryFs {
    async fn relocate(
        &self,
        from: MappedName<'_>,
        to: MappedName<'_>,
    ) -> Result<(), RelocateError> {
        lock(&self.state).relocate(from, to)
    }

    async fn exchange(
        &self,
        one: MappedName<'_>,
        other: MappedName<'_>,
    ) -> Result<(), RelocateError> {
        lock(&self.state).exchange(one, other)
    }
}
