use std::fs;
use std::path::{Path, PathBuf};

use crate::manifest::{Manifest, MANIFEST_FILE};
use crate::{Error, Result};

/// A fixture set being read.
pub struct FixtureReader {
    root: PathBuf,
    manifest: Manifest,
}

impl FixtureReader {
    /// Opens a set and reads its manifest, insisting on full coverage first.
    pub fn open(root: &Path) -> Result<Self> {
        let path = root.join(MANIFEST_FILE);
        let json = fs::read_to_string(&path).map_err(Error::read(&path))?;
        let manifest: Manifest =
            serde_json::from_str(&json).map_err(|cause| Error::ManifestUnparsed { path, cause })?;
        manifest.check_coverage()?;
        Ok(Self {
            root: root.to_path_buf(),
            manifest,
        })
    }

    /// What the set states about itself.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Reads one file the manifest points at.
    ///
    /// The path is taken apart and rebuilt rather than joined as given, so a
    /// manifest cannot send a reader outside the set it describes.
    pub fn read(&self, relative: &str) -> Result<Vec<u8>> {
        let mut path = self.root.clone();
        for segment in relative.split('/') {
            if segment.is_empty() || segment == "." || segment == ".." {
                return Err(Error::OutsideSet {
                    relative: relative.to_owned(),
                });
            }
            path.push(segment);
        }
        fs::read(&path).map_err(Error::read(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture_set::FixtureWriter;

    #[test]
    fn a_path_that_leaves_the_set_is_rejected() {
        let directory = tempfile::tempdir().expect("a temporary directory is available");
        FixtureWriter::create(directory.path()).expect("the set is created");
        let reader = FixtureReader {
            root: directory.path().to_path_buf(),
            manifest: serde_json::from_str(
                r#"{"schema":1,"producer":"test","master_key":"","passphrase":"",
                    "containers":[],"control_objects":[],"key_envelopes":[],
                    "stored_master_keys":[],"recovery_codes":[]}"#,
            )
            .expect("the manifest parses"),
        };
        let refused = reader
            .read("../secrets")
            .expect_err("the path leaves the set");
        assert!(
            matches!(&refused, Error::OutsideSet { relative } if relative == "../secrets"),
            "{refused:?}"
        );
    }
}
