use coffret_model::EntryPath;

use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Whether every Entry standing directly in `folder` is on this device;
    /// `None` is the Library root.
    ///
    /// The question a page opened from disk asks before anything is brought
    /// over around it: a folder this device has whole has nothing left to fill,
    /// and one it does not is a folder somebody has started reading. One read
    /// of the catalog — the Entries under the folder this device does not
    /// have — and nothing of Storage, as every question in this module is.
    ///
    /// It says nothing about mappings. An Entry no mapping reaches is never
    /// present, so a folder of them is never whole here; what a caller does
    /// about one is decided where the folder's own listing says whether it is
    /// [`mapped`](super::FolderListing::mapped).
    pub async fn wholly_present(&self, folder: Option<&EntryPath>) -> Result<bool> {
        Ok(self.remote_in(folder).await?.is_empty())
    }
}
