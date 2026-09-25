use std::io;

use coffret_model::EntryPath;
use coffret_usecase::fetch::local_place_of;

use crate::error::{Error, Result};
use crate::local_file::LocalFile;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Opens the current Entry's local file through its configured mapping.
    ///
    /// `None` means the catalog's materialization record outlived the file.
    /// Descendant symbolic links and nonregular files are refusals.
    ///
    /// # Errors
    ///
    /// [`Error::LocalFileNotOpened`](crate::Error::LocalFileNotOpened) for
    /// everything the EP-9 translation refuses on the way to the file:
    /// `UnmappedEntryPath` where no mapping of this device reaches the path,
    /// `UnmaterializablePath` where one does and no file here can stand for it
    /// (spec: EP-2, EP-4), and
    /// `EntryNotCurrent` where the Library holds no current Entry at the path —
    /// a row this device wrote that has outlived the Entry, which a caller acts
    /// on rather than fails at (spec: EP-10). A caller telling one of these from
    /// another does so by the [`FetchError`](crate::FetchError) inside it.
    ///
    /// Not `Fetch`, although the vocabulary inside it is the fetch's: this asks
    /// the translation of an Entry the catalog already says is standing in the
    /// mapped folder, so no transfer is begun or intended, and a chain starting
    /// "the fetch did not finish" would send whoever read it looking for one.
    ///
    /// [`Error::Index`](crate::Error::Index) where the catalog could not be
    /// read, which decided nothing about the file.
    ///
    /// [`Error::Local`](crate::Error::Local) for the disk's own refusals, which
    /// travel as this device's disk answers them rather than being restated
    /// here.
    pub async fn open_local_file(&self, path: &EntryPath) -> Result<Option<LocalFile>> {
        // Built rather than converted: a bare `?` on this vocabulary means
        // `Error::Fetch`, and it would compile — on the busiest of the callers
        // that speak the fetch's vocabulary without fetching.
        let place = local_place_of(self.index.as_ref(), path)
            .await
            .map_err(Error::local_file_not_opened)?;
        match place.open(self.local_fs.as_ref()).await {
            Ok(reader) => Ok(Some(LocalFile::new(reader))),
            Err(refused) if refused.cause.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(refused) => Err(Error::from(refused)),
        }
    }
}
