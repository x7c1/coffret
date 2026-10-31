use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use coffret_model::Redacted;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::appearance::Appearance;
use crate::error::Result;
use crate::library_dir::state_directory;
use crate::owner_only;

/// What the file is called, beside `libraries/` and `accounts/` in the state
/// directory.
const FILE_NAME: &str = "explorer.json";

/// Where this device keeps which set of colours its explorer is drawn in.
///
/// A file of the device's own, in the state directory beside the Libraries
/// (see [`state_directory`]), and not the browser's storage: the desktop app
/// serves the explorer on a port the operating system picks at each launch, so
/// its page has a new origin every time and what it kept in browser storage is
/// not there at the next launch.
///
/// A choice that cannot be read — no file yet, a file that does not parse, a
/// state directory that cannot be found — is [`Appearance::System`], which is
/// what the explorer would be drawn in had nobody chosen: how it looks is
/// never a reason for it not to open.
#[derive(Debug, Clone)]
pub struct AppearanceFile {
    /// The file, or `None` for the one in this device's state directory, found
    /// when it is asked for.
    path: Option<PathBuf>,
}

/// The file's contents.
#[derive(Serialize, Deserialize)]
struct Kept {
    appearance: Appearance,
}

impl AppearanceFile {
    /// The file in this device's state directory.
    pub fn of_this_device() -> Self {
        Self { path: None }
    }

    /// The file at `path` instead: for a case, which must not reach the state
    /// directory of whoever runs it.
    pub fn at(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn path(&self) -> Result<PathBuf> {
        match &self.path {
            Some(path) => Ok(path.clone()),
            None => Ok(state_directory()?.join(FILE_NAME)),
        }
    }

    /// The choice kept here, or [`Appearance::System`] where there is none that
    /// can be read.
    pub fn read(&self) -> Appearance {
        let path = match self.path() {
            Ok(path) => path,
            Err(error) => {
                warn!(
                    operation = "read_appearance",
                    error = %error.redacted(),
                    "the explorer's appearance could not be found; it follows the system",
                );
                return Appearance::System;
            }
        };
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(cause) if cause.kind() == ErrorKind::NotFound => return Appearance::System,
            Err(cause) => {
                warn!(
                    operation = "read_appearance",
                    error_kind = ?cause.kind(),
                    "the explorer's appearance could not be read; it follows the system",
                );
                return Appearance::System;
            }
        };
        match serde_json::from_slice::<Kept>(&bytes) {
            Ok(kept) => kept.appearance,
            Err(cause) => {
                warn!(
                    operation = "read_appearance",
                    error_category = ?cause.classify(),
                    "the explorer's appearance is not one this build reads; it follows the system",
                );
                Appearance::System
            }
        }
    }

    /// Keeps `appearance` as the choice, replacing whatever was kept.
    ///
    /// Written whole to a neighbour and renamed over the file, as the device's
    /// other files are, so a run that dies mid-write leaves the previous choice
    /// rather than half of one.
    pub fn write(&self, appearance: Appearance) -> Result<()> {
        let path = self.path()?;
        if let Some(parent) = path.parent() {
            owner_only::create_dir(parent)?;
        }
        let mut document = serde_json::to_vec_pretty(&Kept { appearance })
            .expect("an appearance always serializes");
        document.push(b'\n');
        owner_only::write_file(&path, &document)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_a_fresh_directory() -> (tempfile::TempDir, AppearanceFile) {
        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        let file = AppearanceFile::at(directory.path().join("state").join(FILE_NAME));
        (directory, file)
    }

    // A device that never chose follows the system.
    #[test]
    fn nothing_kept_reads_as_the_system() {
        let (_directory, file) = in_a_fresh_directory();
        assert_eq!(file.read(), Appearance::System);
    }

    // What was written is what the next reader reads — another process
    // included, which is all a new launch of the app is.
    #[test]
    fn a_choice_written_is_read_back_by_a_fresh_reader() {
        let (directory, file) = in_a_fresh_directory();
        file.write(Appearance::Dark).expect("the choice is kept");
        let again = AppearanceFile::at(directory.path().join("state").join(FILE_NAME));
        assert_eq!(again.read(), Appearance::Dark);

        again.write(Appearance::System).expect("the choice is kept");
        assert_eq!(file.read(), Appearance::System);
    }

    // A file this build cannot read leaves the explorer drawn as it would be
    // had nobody chosen, rather than refusing to say.
    #[test]
    fn a_file_that_does_not_parse_reads_as_the_system() {
        let (directory, file) = in_a_fresh_directory();
        fs::create_dir_all(directory.path().join("state")).expect("the directory is made");
        fs::write(
            directory.path().join("state").join(FILE_NAME),
            "{\"appearance\":\"sepia\"}",
        )
        .expect("the file is written");
        assert_eq!(file.read(), Appearance::System);
    }

    // The file is the device's own, kept as its other files are.
    #[cfg(unix)]
    #[test]
    fn the_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let (directory, file) = in_a_fresh_directory();
        file.write(Appearance::Light).expect("the choice is kept");
        let mode = fs::metadata(directory.path().join("state").join(FILE_NAME))
            .expect("the file is there")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, owner_only::OWNER_ONLY_FILE);
    }
}
