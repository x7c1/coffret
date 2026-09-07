use std::fs::Metadata;
use std::io::ErrorKind;
use std::path::Path;

use async_trait::async_trait;
use coffret_usecase::device_state::RootIdentity;
use coffret_usecase::{
    FolderEntry, FolderEntryKind, LocalIoError, LocalOperation, MappedRoots, RootProbe,
    SourceReader,
};
use tokio::fs;
use tracing::debug;

use crate::local_times::{btime_of, mtime_of};
use crate::unix_fs::UnixFs;
use crate::unix_source_reader::UnixSourceReader;

#[async_trait]
impl MappedRoots for UnixFs {
    async fn probe_root(&self, root: &Path) -> Result<Option<RootProbe>, LocalIoError> {
        let metadata = match fs::metadata(root).await {
            Ok(metadata) => metadata,
            // Absence is the outcome the caller has a verdict for, and reading
            // an error kind to find that out is what the capability exists to
            // keep out of the layer above (spec: EP-12).
            Err(error) if error.kind() == ErrorKind::NotFound => {
                debug!("a mapped root was not there when it was stated");
                return Ok(None);
            }
            Err(cause) => {
                return Err(LocalIoError::new(LocalOperation::Stating, root, cause));
            }
        };
        Ok(Some(RootProbe {
            identity: identity_of(&metadata),
        }))
    }

    async fn list_folder(&self, dir: &Path) -> Result<Option<Vec<FolderEntry>>, LocalIoError> {
        let mut listing = match fs::read_dir(dir).await {
            Ok(listing) => listing,
            // A folder that is not there holds no files, which is a verdict the
            // walk above has — a subfolder that went away mid-walk, or a root
            // that went between the probe and this call.
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(cause) => return Err(LocalIoError::new(LocalOperation::Listing, dir, cause)),
        };

        let mut entries = Vec::new();
        while let Some(child) = listing
            .next_entry()
            .await
            .map_err(|cause| LocalIoError::new(LocalOperation::Listing, dir, cause))?
        {
            let path = child.path();
            // `symlink_metadata` and not `metadata`: a symbolic link is neither
            // followed nor given an Entry Path of its own, so what is wanted is
            // the link itself and never what it points at (spec: EP-8).
            let metadata = match fs::symlink_metadata(&path).await {
                Ok(metadata) => metadata,
                // A child that went between the listing and its stat is no
                // longer there to carry into the Library, so it is left out
                // rather than failing the walk.
                Err(error) if error.kind() == ErrorKind::NotFound => continue,
                Err(cause) => return Err(LocalIoError::new(LocalOperation::Stating, path, cause)),
            };
            entries.push(FolderEntry {
                name: child.file_name(),
                kind: kind_of(&metadata),
            });
        }
        Ok(Some(entries))
    }

    async fn open_source(&self, path: &Path) -> Result<Box<dyn SourceReader>, LocalIoError> {
        let file = fs::File::open(path)
            .await
            .map_err(|cause| LocalIoError::new(LocalOperation::Reading, path, cause))?;
        Ok(Box::new(UnixSourceReader::new(file, path.to_path_buf())))
    }
}

/// What one directory entry stands for, with links unfollowed (spec: EP-8).
///
/// The times come off the same metadata the kind does, because a second stat
/// would answer about a file that may already have moved — and a birth time
/// cannot be read at all once the file is gone (spec: FM-9).
fn kind_of(metadata: &Metadata) -> FolderEntryKind {
    if metadata.is_dir() {
        FolderEntryKind::Folder
    } else if metadata.is_file() {
        FolderEntryKind::File {
            size: metadata.len(),
            mtime: mtime_of(metadata),
            btime: btime_of(metadata),
        }
    } else {
        FolderEntryKind::Other
    }
}

/// What this platform can say about the filesystem one folder stands on, or
/// `None` where it can say nothing (spec: EP-12).
///
/// The `unix-dev:` tag is not decoration: it is what keeps a value one platform
/// recorded from ever comparing equal to a value another platform's form
/// happened to spell the same way, which matters because the comparison is what
/// decides whether deletion inference runs.
#[cfg(unix)]
fn identity_of(metadata: &Metadata) -> Option<RootIdentity> {
    use std::os::unix::fs::MetadataExt;

    Some(RootIdentity::new(format!("unix-dev:{}", metadata.dev())))
}

#[cfg(not(unix))]
fn identity_of(_metadata: &Metadata) -> Option<RootIdentity> {
    None
}
