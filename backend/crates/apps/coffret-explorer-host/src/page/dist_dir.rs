use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{Assets, Lookup};

/// The explorer's build output in a directory on disk, read afresh for every
/// request.
///
/// Every component of a name is looked at on the way down, without following
/// it: a symbolic link anywhere below the root, to a file or to a directory,
/// is refused rather than served, since what it points at is outside what the
/// build put there. The root itself is taken as it was named.
pub(crate) struct DistDir {
    root: PathBuf,
}

impl DistDir {
    pub(super) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// The bytes of `name`'s file, or why there are none.
    fn read(&self, name: &str) -> io::Result<Lookup> {
        let mut path = self.root.clone();
        for segment in name.split('/') {
            path.push(segment);
            let kind = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata.file_type(),
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                    return Ok(Lookup::Missing)
                }
                Err(cause) => return Err(cause),
            };
            if kind.is_symlink() {
                return Ok(Lookup::Refused);
            }
        }
        if !fs::symlink_metadata(&path)?.is_file() {
            // A directory, which names no file of its own: a path the explorer
            // reads itself, like any other that names none.
            return Ok(Lookup::Missing);
        }
        Ok(Lookup::Found(fs::read(&path)?.into()))
    }

    /// The directory this serves.
    pub(super) fn root(&self) -> &Path {
        &self.root
    }
}

impl Assets for DistDir {
    fn file(&self, name: &str) -> Lookup {
        if name.is_empty() {
            return Lookup::Missing;
        }
        match self.read(name) {
            Ok(lookup) => lookup,
            Err(cause) => {
                // The kind alone: the error's message may name the file
                // (spec: EL-3).
                tracing::warn!(
                    io_error_kind = ?cause.kind(),
                    "a file of the explorer's build on disk could not be read",
                );
                Lookup::Refused
            }
        }
    }
}
