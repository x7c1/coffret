use std::path::{Path, PathBuf};

use coffret_model::EntryPath;

use crate::MappedRelativeLocation;

/// One folder below a configured mapped root, or that root itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFolder {
    root: PathBuf,
    relative: Option<MappedRelativeLocation>,
}

impl LocalFolder {
    pub(super) fn root(root: PathBuf) -> Self {
        Self {
            root,
            relative: None,
        }
    }

    pub(super) fn below(root: PathBuf, relative: &EntryPath) -> Self {
        Self {
            root,
            relative: Some(MappedRelativeLocation::from_entry_path(relative)),
        }
    }

    /// The configured mapped root.
    pub fn mapped_root(&self) -> &Path {
        &self.root
    }

    /// The validated local-relative location, absent for the root itself.
    pub fn relative(&self) -> Option<&MappedRelativeLocation> {
        self.relative.as_ref()
    }
}
