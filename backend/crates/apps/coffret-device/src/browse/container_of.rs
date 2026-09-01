use coffret_model::{ContainerKind, EntryPath};
use tracing::warn;

use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Which kind of Container holds the current Entry at one path, or `None`
    /// where the Library holds no Entry there (spec: PK-15).
    ///
    /// The single-path form of what a listing's rows carry, and it is asked for
    /// the reason that field exists: an Entry inside a Pack cannot be replaced
    /// one file at a time, so whatever offers to write over it has to know before
    /// it offers (spec: PK-10, PK-12). A caller with a listing in hand has the
    /// answer already; this is for one that has a path and no listing — a file
    /// arriving at a subpath of the folder somebody is looking at, which is not a
    /// row of that folder at all.
    ///
    /// Two reads and not one, because the catalog answers them separately: where
    /// the Entry lives, and what the Container holding it is. Another process may
    /// commit between them, which leaves an Entry with no summary of its
    /// Container — rare, nobody's mistake, and gone by the next read. It comes
    /// back as the kind that refuses nothing on its own, exactly as a listing's
    /// row does, because the alternative is refusing a file over a Container that
    /// is no longer there.
    pub async fn container_of(&self, path: &EntryPath) -> Result<Option<ContainerKind>> {
        let Some(location) = self.index.entry_at(path).await? else {
            return Ok(None);
        };
        let kind = self
            .index
            .containers_under(Some(path))
            .await?
            .into_iter()
            .find(|container| container.id == location.container_id)
            .map(|container| container.kind);
        if kind.is_none() {
            warn!(
                operation = "container_of",
                container = %location.container_id,
                "the catalog holds an Entry in a Container it summarizes no longer",
            );
        }
        Ok(Some(kind.unwrap_or(ContainerKind::OneFile)))
    }
}
