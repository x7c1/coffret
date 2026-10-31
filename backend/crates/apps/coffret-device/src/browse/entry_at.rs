use coffret_model::EntryPath;
use coffret_usecase::commit::EntryRef;

use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Which Entry the Library holds at `path` now, by what no rename changes
    /// (spec: FM-9), or `None` where it holds none.
    ///
    /// Two answers asked at two moments are the same Entry exactly where
    /// nothing committed at the path in between: a commit that replaces the
    /// file there makes an Entry of another Container (spec: PK-11). So a
    /// caller keeping account of what stood at a path before it wrote there
    /// can tell, afterwards, whether a run has committed what it wrote
    /// (spec: LA-36).
    pub async fn entry_at(&self, path: &EntryPath) -> Result<Option<EntryRef>> {
        Ok(self.index.entry_at(path).await?.as_ref().map(EntryRef::of))
    }
}
