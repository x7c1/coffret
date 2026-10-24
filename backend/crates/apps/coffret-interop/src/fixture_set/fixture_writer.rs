use std::fs;
use std::path::{Path, PathBuf};

use crate::manifest::{Manifest, MANIFEST_FILE};
use crate::{Error, Result};

use super::{BLOBS_DIR, OBJECTS_DIR};

/// A fixture set being written.
pub struct FixtureWriter {
    root: PathBuf,
}

impl FixtureWriter {
    /// Prepares an empty directory to write a set into.
    pub fn create(root: &Path) -> Result<Self> {
        for directory in [root.join(OBJECTS_DIR), root.join(BLOBS_DIR)] {
            fs::create_dir_all(&directory).map_err(|cause| Error::CreateDirectory {
                path: directory,
                cause,
            })?;
        }
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    /// Writes one file, returning the manifest-relative path to it.
    pub fn write(&self, directory: &str, name: &str, bytes: &[u8]) -> Result<String> {
        let relative = format!("{directory}/{name}");
        let path = self.root.join(directory).join(name);
        fs::write(&path, bytes).map_err(Error::write(&path))?;
        Ok(relative)
    }

    /// Writes the manifest, which is the last thing a complete set gains.
    pub fn write_manifest(&self, manifest: &Manifest) -> Result<()> {
        let path = self.root.join(MANIFEST_FILE);
        let mut json = serde_json::to_string_pretty(manifest)
            .map_err(|cause| Error::ManifestUnserialized { cause })?;
        json.push('\n');
        fs::write(&path, json).map_err(Error::write(path))
    }
}
