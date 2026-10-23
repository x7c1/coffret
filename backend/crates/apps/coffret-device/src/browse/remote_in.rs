use coffret_model::{EntryLocation, EntryPath};

use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// The Entries standing directly in `folder` that this device does not
    /// have, in EP-3 order — the rows a listing calls `remote`.
    ///
    /// One read of the catalog, which answers with the subtree's Entries this
    /// device does not have; the ones standing deeper are left out here.
    pub(crate) async fn remote_in(&self, folder: Option<&EntryPath>) -> Result<Vec<EntryLocation>> {
        Ok(self
            .index
            .entries_absent_under(folder)
            .await?
            .into_iter()
            .filter(|location| location.path().parent().as_ref() == folder)
            .collect())
    }
}
