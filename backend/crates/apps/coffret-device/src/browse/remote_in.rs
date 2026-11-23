use coffret_model::{EntryLocation, EntryPath};

use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// The Entries standing directly in `folder` that this device does not
    /// have, in EP-3 order — the rows a listing calls `remote`.
    ///
    /// Read off what the folder holds one level down
    /// ([`folder_contents`](coffret_usecase::Index::folder_contents)), the
    /// same answer the listing is drawn from, so nothing below the folder's
    /// child folders is read however much lies there. An Entry
    /// is had by the same present materialization record the listing's
    /// `present` reads (spec: EP-10).
    pub(crate) async fn remote_in(&self, folder: Option<&EntryPath>) -> Result<Vec<EntryLocation>> {
        Ok(self
            .index
            .folder_contents(folder)
            .await?
            .files
            .into_iter()
            .filter(|file| !file.present)
            .map(|file| file.location)
            .collect())
    }
}
