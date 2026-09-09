use std::path::PathBuf;

use coffret_model::EntryPath;
use coffret_usecase::fetch::local_path_of;

use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Where on this device the file for the Entry at `path` belongs
    /// (spec: EP-9).
    ///
    /// This joined path is for display and reporting. A shell asks this call
    /// rather than rederiving EP-9 from the mappings itself. Reading the file uses
    /// [`open_local_file`](Self::open_local_file), which keeps the mapped root and
    /// validated relative location separate during descriptor descent
    /// (spec: EP-8).
    ///
    /// It is where the file *belongs* and never a claim that it is there:
    /// [`state_of`](Self::state_of) is what says whether this device has it
    /// (spec: EP-10).
    ///
    /// # Errors
    ///
    /// [`Error::Fetch`](crate::Error::Fetch) carrying `EntryNotCurrent` where
    /// the Library holds no current Entry at the path, `UnmappedEntryPath` where
    /// it holds one that no mapping of this device reaches,
    /// `UnmaterializablePath` where a mapping does reach it and no file here can
    /// stand for it (spec: EP-2, EP-4), and `Index` where the catalog could not
    /// be read at all. A shell telling one of these from another does so by the
    /// [`FetchError`](crate::FetchError) it carries, which is why this crate
    /// re-exports that type.
    pub async fn local_path_of(&self, path: &EntryPath) -> Result<PathBuf> {
        Ok(local_path_of(self.index.as_ref(), path).await?)
    }
}
