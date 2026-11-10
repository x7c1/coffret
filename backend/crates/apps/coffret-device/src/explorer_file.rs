use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use coffret_model::Redacted;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};
use tracing::warn;

use crate::appearance::Appearance;
use crate::error::Result;
use crate::filter_settings::FilterSettings;
use crate::library_dir::state_directory;
use crate::owner_only;

/// What the file is called, beside `libraries/` and `accounts/` in the state
/// directory.
const FILE_NAME: &str = "explorer.json";

/// The field the appearance is kept under (spec: LA-38).
const APPEARANCE: &str = "appearance";

/// The field the filter settings are kept under (spec: LA-43).
const FILTER: &str = "filter";

/// Where this device keeps its explorer's own state: which set of colours it
/// is drawn in (spec: LA-38) and what its tree was last filtered by (spec:
/// LA-43).
///
/// A file of the device's own, in the state directory beside the Libraries
/// (see [`state_directory`]), and not the browser's storage: the desktop app
/// serves the explorer on a port the operating system picks at each launch, so
/// its page has a new origin every time and what it kept in browser storage is
/// not there at the next launch.
///
/// The file is one JSON object with a field for each thing kept, and each is
/// read on its own: a field that cannot be read — no file yet, a file that
/// does not parse, a field this build does not read, a state directory that
/// cannot be found — is what the explorer would show had nobody chosen
/// ([`Appearance::System`], [`FilterSettings::none`]), and leaves the other
/// field as it reads. How it looks, or how it was last filtered, is never a
/// reason for it not to open.
///
/// Keeping one field reads the file, changes that field and writes the whole
/// back, so the other field stays as it was — one this build cannot read
/// included. Clones of one `ExplorerFile` share a lock around that, so two
/// keeps in one process do not lose one another's field.
#[derive(Debug, Clone)]
pub struct ExplorerFile {
    /// The file, or `None` for the one in this device's state directory, found
    /// when it is asked for.
    path: Option<PathBuf>,
    /// Held across a keep's read and write.
    keeping: Arc<Mutex<()>>,
}

impl ExplorerFile {
    /// The file in this device's state directory.
    pub fn of_this_device() -> Self {
        Self {
            path: None,
            keeping: Arc::default(),
        }
    }

    /// The file at `path` instead: for a case, which must not reach the state
    /// directory of whoever runs it.
    pub fn at(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            keeping: Arc::default(),
        }
    }

    fn path(&self) -> Result<PathBuf> {
        match &self.path {
            Some(path) => Ok(path.clone()),
            None => Ok(state_directory()?.join(FILE_NAME)),
        }
    }

    /// The appearance kept here, or [`Appearance::System`] where there is none
    /// that can be read.
    pub fn appearance(&self) -> Appearance {
        self.field(APPEARANCE, "read_appearance", "the explorer's appearance")
            .unwrap_or_default()
    }

    /// Keeps `appearance` as the choice, replacing whatever was kept, and
    /// leaving the filter settings as they were.
    pub fn keep_appearance(&self, appearance: Appearance) -> Result<()> {
        self.keep(APPEARANCE, &appearance)
    }

    /// The filter settings kept here, or [`FilterSettings::none`] where there
    /// are none that can be read.
    pub fn filter_settings(&self) -> FilterSettings {
        self.field(
            FILTER,
            "read_filter_settings",
            "the explorer's filter settings",
        )
        .unwrap_or_else(FilterSettings::none)
    }

    /// Keeps `settings` as the filter, replacing whatever was kept, and
    /// leaving the appearance as it was.
    pub fn keep_filter_settings(&self, settings: &FilterSettings) -> Result<()> {
        self.keep(FILTER, settings)
    }

    /// The field `key` of the file, or `None` where it is not there or cannot
    /// be read, which is said in the log under `operation`.
    fn field<T: DeserializeOwned>(&self, key: &str, operation: &str, what: &str) -> Option<T> {
        let path = match self.path() {
            Ok(path) => path,
            Err(error) => {
                warn!(
                    operation,
                    error = %error.redacted(),
                    "{what} could not be found; it is taken as never chosen",
                );
                return None;
            }
        };
        let mut document = read_document(&path, operation, what)?;
        let value = document.remove(key)?;
        match serde_json::from_value(value) {
            Ok(field) => Some(field),
            Err(cause) => {
                warn!(
                    operation,
                    error_category = ?cause.classify(),
                    "{what} is not one this build reads; it is taken as never chosen",
                );
                None
            }
        }
    }

    /// Writes `value` as the field `key`, keeping every other field the file
    /// holds.
    ///
    /// Written whole to a neighbour and renamed over the file, as the device's
    /// other files are, so a run that dies mid-write leaves what was kept
    /// before rather than half of it.
    fn keep(&self, key: &str, value: &impl Serialize) -> Result<()> {
        let path = self.path()?;
        let _keeping = self
            .keeping
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut document =
            read_document(&path, "keep_explorer_state", "the explorer's state").unwrap_or_default();
        document.insert(
            key.to_owned(),
            serde_json::to_value(value).expect("what the explorer keeps always serializes"),
        );
        if let Some(parent) = path.parent() {
            owner_only::create_dir(parent)?;
        }
        let mut bytes = serde_json::to_vec_pretty(&Value::Object(document))
            .expect("a JSON object always serializes");
        bytes.push(b'\n');
        owner_only::write_file(&path, &bytes)
    }
}

/// The file at `path` as an object, or `None` where there is none or it cannot
/// be read as one, which is said in the log unless there is simply no file.
fn read_document(
    path: &std::path::Path,
    operation: &str,
    what: &str,
) -> Option<Map<String, Value>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(cause) if cause.kind() == ErrorKind::NotFound => return None,
        Err(cause) => {
            warn!(
                operation,
                error_kind = ?cause.kind(),
                "{what} could not be read; it is taken as never chosen",
            );
            return None;
        }
    };
    match serde_json::from_slice::<Map<String, Value>>(&bytes) {
        Ok(document) => Some(document),
        Err(cause) => {
            warn!(
                operation,
                error_category = ?cause.classify(),
                "{what} is not a file this build reads; it is taken as never chosen",
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_a_fresh_directory() -> (tempfile::TempDir, ExplorerFile) {
        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        let file = ExplorerFile::at(directory.path().join("state").join(FILE_NAME));
        (directory, file)
    }

    fn written(directory: &tempfile::TempDir, contents: &str) {
        fs::create_dir_all(directory.path().join("state")).expect("the directory is made");
        fs::write(directory.path().join("state").join(FILE_NAME), contents)
            .expect("the file is written");
    }

    fn named(name: &str, recent_on: bool, recent: u32) -> FilterSettings {
        FilterSettings::new(name.to_owned(), recent_on, recent).expect("the settings are valid")
    }

    // A device that never chose follows the system, with no filter.
    #[test]
    fn nothing_kept_reads_as_never_chosen() {
        let (_directory, file) = in_a_fresh_directory();
        assert_eq!(file.appearance(), Appearance::System);
        assert_eq!(file.filter_settings(), FilterSettings::none());
    }

    // What was written is what the next reader reads — another process
    // included, which is all a new launch of the app is.
    #[test]
    fn a_choice_written_is_read_back_by_a_fresh_reader() {
        let (directory, file) = in_a_fresh_directory();
        file.keep_appearance(Appearance::Dark)
            .expect("the choice is kept");
        let again = ExplorerFile::at(directory.path().join("state").join(FILE_NAME));
        assert_eq!(again.appearance(), Appearance::Dark);

        again
            .keep_appearance(Appearance::System)
            .expect("the choice is kept");
        assert_eq!(file.appearance(), Appearance::System);
    }

    // LA-43: keeping one field leaves the other as it was.
    #[test]
    fn keeping_one_field_leaves_the_other() {
        let (_directory, file) = in_a_fresh_directory();
        file.keep_appearance(Appearance::Light)
            .expect("the choice is kept");
        file.keep_filter_settings(&named("cat", true, 3))
            .expect("the filter is kept");
        assert_eq!(file.appearance(), Appearance::Light);
        assert_eq!(file.filter_settings(), named("cat", true, 3));

        file.keep_appearance(Appearance::Dark)
            .expect("the choice is kept");
        assert_eq!(file.filter_settings(), named("cat", true, 3));
    }

    // LA-43: a file written before the filter was kept reads, with no filter.
    #[test]
    fn a_file_holding_the_appearance_alone_reads_with_no_filter() {
        let (directory, file) = in_a_fresh_directory();
        written(&directory, "{\n  \"appearance\": \"dark\"\n}\n");
        assert_eq!(file.appearance(), Appearance::Dark);
        assert_eq!(file.filter_settings(), FilterSettings::none());
    }

    // A file this build cannot read leaves the explorer drawn as it would be
    // had nobody chosen, rather than refusing to say — one field at a time.
    #[test]
    fn a_field_that_does_not_parse_reads_as_never_chosen() {
        let (directory, file) = in_a_fresh_directory();
        written(
            &directory,
            r#"{"appearance":"sepia","filter":{"name":"cat","recentOn":true,"recent":4}}"#,
        );
        assert_eq!(file.appearance(), Appearance::System);
        assert_eq!(file.filter_settings(), named("cat", true, 4));

        written(
            &directory,
            r#"{"appearance":"dark","filter":{"name":"cat","recentOn":true,"recent":0}}"#,
        );
        assert_eq!(file.appearance(), Appearance::Dark);
        assert_eq!(file.filter_settings(), FilterSettings::none());

        written(&directory, "not json");
        assert_eq!(file.appearance(), Appearance::System);
        assert_eq!(file.filter_settings(), FilterSettings::none());
    }

    // A file that does not parse is written over whole when something is
    // kept, rather than refusing to keep anything.
    #[test]
    fn a_file_that_does_not_parse_is_written_over() {
        let (directory, file) = in_a_fresh_directory();
        written(&directory, "not json");
        file.keep_filter_settings(&named("cat", false, 10))
            .expect("the filter is kept");
        assert_eq!(file.filter_settings(), named("cat", false, 10));
        assert_eq!(file.appearance(), Appearance::System);
    }

    // The file is the device's own, kept as its other files are.
    #[cfg(unix)]
    #[test]
    fn the_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let (directory, file) = in_a_fresh_directory();
        file.keep_appearance(Appearance::Light)
            .expect("the choice is kept");
        let mode = fs::metadata(directory.path().join("state").join(FILE_NAME))
            .expect("the file is there")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, owner_only::OWNER_ONLY_FILE);
    }
}
