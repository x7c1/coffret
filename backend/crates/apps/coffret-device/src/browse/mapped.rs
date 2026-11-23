use coffret_model::EntryPath;

use super::reach::Reach;
use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Whether a mapping of this device reaches one folder, `None` being the
    /// Library root: whether a file could be put there (spec: EP-9).
    ///
    /// The listing's own [`mapped`](super::FolderListing::mapped), answered
    /// from the mappings alone: a drop, an import, a freeze or a fill asks it
    /// before anything else and wants nothing else the listing reads.
    pub async fn mapped(&self, folder: Option<&EntryPath>) -> Result<bool> {
        Ok(Reach::of(self.index.mappings().await?).reaches(folder))
    }
}
