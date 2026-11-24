use coffret_model::EntryPath;
use tracing::debug;

use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Every folder in the Library, in EP-3 order: what `GET /api/folders`
    /// answers (spec: LA-47).
    ///
    /// Flat and complete: each folder is named by its whole path, and a folder
    /// implied by an Entry Path several components deep contributes every
    /// ancestor along the way. Whatever nests them into a tree does so from
    /// this, which is what keeps the nesting out of the catalog — where there
    /// are no folders to nest.
    ///
    /// A folder exists exactly where a current Entry stands under it. There are
    /// no empty ones, because a Library holds Entries and nothing else: a folder
    /// whose last Entry is removed stops existing at that commit.
    ///
    /// The Index answers with the folders it keeps apart rather than out of
    /// every Entry, so the answer costs what it holds (spec: LA-45).
    pub async fn folders(&self) -> Result<Vec<EntryPath>> {
        let folders = self.index.folders().await?;
        debug!(
            operation = "folders",
            library = %self.library_id,
            folders = folders.len(),
            "read the Library's folders from the catalog",
        );
        Ok(folders)
    }
}
