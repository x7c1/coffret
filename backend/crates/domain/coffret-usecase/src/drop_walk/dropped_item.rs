use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// One thing a drop let go of: a name in a folder on this device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedItem {
    /// The folder it stands in, as the operating system was handed it.
    pub parent: PathBuf,
    /// Its own name in that folder.
    pub name: OsString,
}

impl DroppedItem {
    /// The item at `path`, or `None` where the path names no item in a
    /// folder: the root of the filesystem, or a path ending in `..`.
    pub fn at(path: &Path) -> Option<Self> {
        Some(Self {
            parent: path.parent()?.to_path_buf(),
            name: path.file_name()?.to_os_string(),
        })
    }
}
