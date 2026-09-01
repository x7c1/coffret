use coffret_model::EntryPath;
use coffret_usecase::fetch::{local_path_for, FetchError};
use coffret_usecase::scratch;

use super::IncomingFile;
use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Opens the file at one Entry Path for writing, in the folder this device
    /// maps that part of the Library into (spec: EP-9).
    ///
    /// The path is where the file will stand in the Library once a sync has
    /// carried it in, and where it goes on disk is the mappings' answer — the
    /// same translation a fetch makes before placing an Entry, so a file added
    /// here and the same file fetched onto another device land in the folder each
    /// device chose for that subtree.
    ///
    /// Nothing is written by this call and nothing is decided by it beyond the
    /// path: the bytes go through [`IncomingFile`] and the file exists only when
    /// [`keep`](IncomingFile::keep) renames it into place.
    ///
    /// # Errors
    ///
    /// [`Error::Fetch`](crate::Error::Fetch) carrying `UnmappedEntryPath` where
    /// no mapping of this device reaches the path — nowhere on this device stands
    /// for that part of the Library, so there is nowhere to put the file — and
    /// `UnmaterializablePath` where a mapping does reach it and no file here may
    /// stand for it (spec: EP-2, EP-4).
    ///
    /// A component carrying coffret's reserved scratch prefix is among the
    /// second. The prefix is what a scan steps over
    /// ([`scratch`](coffret_usecase::scratch)), so a file written under one would
    /// sit in a mapped folder that no sync will ever carry in — visible, the
    /// person's own, and permanently outside the Library. Refusing the name says
    /// so at the moment it can still be changed, rather than accepting the file
    /// and quietly never backing it up.
    ///
    /// The same `Error::Fetch` carrying `Index` where the mappings could not be
    /// read at all, which is neither verdict about the path — nothing was
    /// decided, so nothing is refused.
    ///
    /// `Local` where the folders above the file could not be made, or the
    /// temporary file could not be created.
    pub async fn receive_file(&self, path: &EntryPath) -> Result<IncomingFile> {
        if path.as_str().split('/').any(scratch::is_scratch) {
            return Err(FetchError::UnmaterializablePath { path: path.clone() }.into());
        }
        let destination = local_path_for(self.index.as_ref(), path).await?;
        IncomingFile::open(destination).await
    }
}
