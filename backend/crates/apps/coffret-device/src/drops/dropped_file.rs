use coffret_local_fs::UnixFs;
use coffret_model::{Btime, Mtime};
use coffret_usecase::drop_walk::DroppedSource;
use coffret_usecase::MappedRoots;

use crate::error::{Error, Result};

use super::DropReader;

/// One file a drop took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedFile {
    source: DroppedSource,
}

impl DroppedFile {
    pub(super) fn new(source: DroppedSource) -> Self {
        Self { source }
    }

    /// Its path relative to the folder the dropped item stood in, as the page
    /// was told it.
    pub fn path(&self) -> &str {
        &self.source.path
    }

    /// Its length when the drop was walked.
    pub fn size(&self) -> u64 {
        self.source.size
    }

    /// Its modification time when the drop was walked (spec: FM-9).
    pub fn mtime(&self) -> Mtime {
        self.source.mtime
    }

    /// Its birth time, where the platform reported one (spec: FM-9).
    pub fn btime(&self) -> Option<Btime> {
        self.source.btime
    }

    /// Opens it for reading, again from the folder the dropped item stood in
    /// and through every component below it without following a link — the
    /// reader a freeze reads a source through (spec: EP-8). A file replaced
    /// since the walk by a link, or by something that is not a regular file,
    /// is refused rather than read.
    pub async fn open(&self) -> Result<DropReader> {
        let reader = UnixFs::new()
            .source_reader(&self.source.root, &self.source.relative)
            .await
            .map_err(Error::Local)?;
        Ok(DropReader::new(reader))
    }
}
