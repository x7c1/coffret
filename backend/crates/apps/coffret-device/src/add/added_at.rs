use std::path::PathBuf;

use coffret_model::EntryPath;
use coffret_usecase::fetch::{local_path_for, FetchError};
use coffret_usecase::scratch;
use tokio::fs;

use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Where the file for one such path is on this device, or `None` where there
    /// is no such file.
    ///
    /// The single-path form of [`added_locally`](Self::added_locally), and it
    /// answers `None` for every reason that one leaves a name out: the Library
    /// holds a current Entry at the path, so the file there is the Entry's and
    /// [`local_path_of`](Self::local_path_of) is what answers about it; no
    /// mapping reaches the path, or no local file can stand for it (spec: EP-9);
    /// the name is coffret's own scratch; or nothing is there at all.
    ///
    /// What it is for is reading such a file. A file the Library does not hold is
    /// still the person's own file, sitting in their own folder, and a reader
    /// that would not open it until a sync had run would be refusing to show
    /// somebody what they had just put there.
    pub async fn added_at(&self, path: &EntryPath) -> Result<Option<PathBuf>> {
        if path.as_str().split('/').any(scratch::is_scratch) {
            return Ok(None);
        }
        if self.index.entry_at(path).await?.is_some() {
            return Ok(None);
        }
        let local = match local_path_for(self.index.as_ref(), path).await {
            Ok(local) => local,
            // Neither is a failure to report: a path this device cannot hold a
            // file at is a path with no file of this device's at it.
            Err(FetchError::UnmappedEntryPath { .. } | FetchError::UnmaterializablePath { .. }) => {
                return Ok(None)
            }
            Err(cause) => return Err(cause.into()),
        };
        Ok(match fs::metadata(&local).await {
            Ok(metadata) if metadata.is_file() => Some(local),
            _ => None,
        })
    }
}
