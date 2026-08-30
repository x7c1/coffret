use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;

use super::DeviceSettings;
use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;

/// Just enough of the file to say whether this build can read the rest.
///
/// Read first and on its own, so that a file from a later build is reported as
/// a version this build does not know rather than as a provider it does not
/// know — which is what it would look like if the whole document were parsed at
/// once and a later version had renamed something.
#[derive(Deserialize)]
struct Shape {
    version: u32,
}

impl DeviceSettings {
    /// Reads the settings of the Library in `dir`.
    pub fn read(dir: &LibraryDir) -> Result<Self> {
        let path = dir.settings_file();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(cause) if cause.kind() == ErrorKind::NotFound => {
                return Err(Error::NoSuchLibrary {
                    name: dir.name().to_owned(),
                    path: dir.path().to_path_buf(),
                })
            }
            Err(cause) => {
                return Err(Error::Local {
                    doing: "reading the settings",
                    path,
                    cause,
                })
            }
        };

        Self::from_json(&path, &bytes)
    }

    /// Reads settings out of the bytes of one settings file.
    ///
    /// `path` is only ever reported: whatever is wrong with a settings file,
    /// the first thing whoever has to fix it needs is which file it was.
    pub(crate) fn from_json(path: &Path, bytes: &[u8]) -> Result<Self> {
        let shape: Shape =
            serde_json::from_slice(bytes).map_err(|cause| Error::MalformedSettings {
                path: path.to_path_buf(),
                cause,
            })?;
        if shape.version != Self::VERSION {
            return Err(Error::UnsupportedSettingsVersion {
                path: path.to_path_buf(),
                version: shape.version,
                expected: Self::VERSION,
            });
        }

        serde_json::from_slice(bytes).map_err(|cause| Error::MalformedSettings {
            path: path.to_path_buf(),
            cause,
        })
    }
}
