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
    pub async fn open_local_file(&self, path: &EntryPath) -> Result<Option<LocalFile>> {
        let place = local_place_of(self.index.as_ref(), path).await?;
        match place.open(self.local_fs.as_ref()).await {
            Ok(reader) => Ok(Some(LocalFile::new(reader))),
            Err(refused) if refused.cause.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(refused) => Err(Error::from(refused)),
        }
    }
}
