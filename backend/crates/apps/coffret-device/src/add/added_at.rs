use std::io;

use coffret_model::{EntryPath, Redacted};
use coffret_usecase::fetch::{local_place_for, FetchError};
use coffret_usecase::{root_marker, scratch};
use tracing::debug;

use crate::error::Result;
use crate::local_file::LocalFile;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Opens the file for one such path on this device, or answers `None` where
    /// there is no such file.
    ///
    /// The single-path form of [`added_locally`](Self::added_locally), and it
    /// answers `None` for every reason that one leaves a name out: the Library
    /// holds a current Entry at the path, so the file there is the Entry's and
    /// [`open_local_file`](Self::open_local_file) is what opens it; no
    /// mapping reaches the path, or no local file can stand for it (spec: EP-9);
    /// a component of the name is coffret's own — its scratch (spec: EP-11) or
    /// its management area (spec: EP-14) — so nothing under it is a local file of
    /// this device's, whatever stands there; or nothing is there at all.
    ///
    /// The look and the open go through the same capabilities a fetch uses,
    /// which is
    /// what makes "nothing is there" mean the same thing here as it does there:
    /// the components are descended from the mapped root one at a time, so a
    /// symbolic link on the way is a path with no file of this device's at it
    /// rather than something to answer through (spec: EP-4, EP-8). A folder or a
    /// link standing at the name itself is not a file either. The later open
    /// repeats that confined descent and keeps the acquired handle, so a name
    /// changed after the look cannot redirect the bytes.
    ///
    /// What it is for is reading such a file. A file the Library does not hold is
    /// still the person's own file, sitting in their own folder, and a reader
    /// that would not open it until a sync had run would be refusing to show
    /// somebody what they had just put there.
    pub async fn added_at(&self, path: &EntryPath) -> Result<Option<LocalFile>> {
        if path.as_str().split('/').any(|component| {
            scratch::is_scratch(component) || root_marker::is_management_area(component)
        }) {
            return Ok(None);
        }
        if self.index.entry_at(path).await?.is_some() {
            return Ok(None);
        }
        let place = match local_place_for(self.index.as_ref(), path).await {
            Ok(place) => place,
            // Neither is a failure to report: a path this device cannot hold a
            // file at is a path with no file of this device's at it.
            Err(FetchError::UnmappedEntryPath { .. } | FetchError::UnmaterializablePath { .. }) => {
                return Ok(None)
            }
            Err(cause) => return Err(cause.into()),
        };
        let standing = match place.look(self.local_fs.as_ref()).await {
            Ok(standing) => standing,
            // A component the descent would not pass through, and a disk that
            // would not answer, are both "no file of this device's here": the
            // question was where a file *is*, and a caller with nothing to open
            // has nothing to do with the shape of the folders above it. Recorded
            // rather than swallowed outright, because the second of the two is a
            // disk that is unwell and this is the only account of it. The
            // refusal goes in through its log-safe rendering, which is what
            // keeps the folder and the file out of the event (spec: EL-1).
            Err(refused) => {
                debug!(
                    operation = "added_at",
                    reason = %refused.redacted(),
                    "no file of this device's stands at a path the Library holds nothing at",
                );
                return Ok(None);
            }
        };
        if !standing.is_some_and(|standing| standing.is_file) {
            return Ok(None);
        }
        match place.open(self.local_fs.as_ref()).await {
            Ok(reader) => Ok(Some(LocalFile::new(reader))),
            Err(refused) if refused.cause.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(refused) => Err(refused.into()),
        }
    }
}
