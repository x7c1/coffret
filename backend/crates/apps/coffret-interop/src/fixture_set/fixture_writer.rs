use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::manifest::{Manifest, MANIFEST_FILE};

use super::{BLOBS_DIR, OBJECTS_DIR};

/// A fixture set being written.
pub struct FixtureWriter {
    root: PathBuf,
}

impl FixtureWriter {
    /// Prepares an empty directory to write a set into.
    pub fn create(root: &Path) -> Result<Self> {
        for directory in [root.join(OBJECTS_DIR), root.join(BLOBS_DIR)] {
            fs::create_dir_all(&directory)
                .with_context(|| format!("creating {}", directory.display()))?;
        }
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    /// Writes one file, returning the manifest-relative path to it.
    pub fn write(&self, directory: &str, name: &str, bytes: &[u8]) -> Result<String> {
        let relative = format!("{directory}/{name}");
        let path = self.root.join(directory).join(name);
        fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
        Ok(relative)
    }

    /// Writes the manifest, which is the last thing a complete set gains.
    pub fn write_manifest(&self, manifest: &Manifest) -> Result<()> {
        let path = self.root.join(MANIFEST_FILE);
        let mut json =
            serde_json::to_string_pretty(manifest).context("serializing the manifest")?;
        json.push('\n');
        fs::write(&path, json).with_context(|| format!("writing {}", path.display()))
    }
}
