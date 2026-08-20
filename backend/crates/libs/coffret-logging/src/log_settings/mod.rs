use std::path::{Path, PathBuf};

use tracing::Level;

mod default_directory;
pub use default_directory::default_directory;

mod from_env;
pub use from_env::{LOG_DIRECTORY, LOG_LEVEL, LOG_MAX_BYTES};

mod keeps;

mod sizes;
use sizes::{DEFAULT_MAX_FILE_BYTES, DEFAULT_MAX_TOTAL_BYTES};

/// Where the events go, how loud they are, whose they are, and how much disk
/// they may use.
///
/// The state directory is the home for it — `$XDG_STATE_HOME/coffret/logs`,
/// falling back to `$HOME/.local/state/coffret/logs` — because losing the log
/// loses the evidence of what a provider answered, which is neither a cache to
/// be dropped nor configuration to be edited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSettings {
    directory: PathBuf,
    max_total_bytes: u64,
    max_file_bytes: u64,
    level: Level,
    extra_targets: Vec<String>,
}

impl LogSettings {
    /// Logs into a directory of the caller's choosing.
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            level: Level::INFO,
            extra_targets: Vec::new(),
        }
    }

    /// Sets the total bytes of log files that may be kept.
    ///
    /// The per-file size follows it down as the files are opened, so that a
    /// ceiling smaller than the default file size still means what it says.
    pub fn with_ceiling(mut self, max_total_bytes: u64) -> Self {
        self.max_total_bytes = max_total_bytes;
        self
    }

    /// Sets how large one file grows before the next one is started.
    pub fn with_max_file_bytes(mut self, max_file_bytes: u64) -> Self {
        self.max_file_bytes = max_file_bytes;
        self
    }

    /// Sets the most verbose level that reaches the file.
    ///
    /// Capped at `DEBUG`, whatever is asked for. Coffret emits nothing below
    /// `DEBUG`, so `TRACE` would turn on nothing of its own and everything of
    /// its dependencies' — an HTTP stack and a cloud SDK, whose instrumentation
    /// prints request headers and signing material, and which follow no rule of
    /// coffret's about what may be written to a file.
    pub fn with_level(mut self, level: Level) -> Self {
        self.level = level.min(Level::DEBUG);
        self
    }

    /// Lets one more crate's events reach the file, alongside coffret's own.
    ///
    /// Only *which* events are kept, never how loud they are: a widened target
    /// is read at the same level as everything else, so this can never be a way
    /// around the `DEBUG` cap that [`with_level`](Self::with_level) applies.
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.extra_targets.push(target.into());
        self
    }

    /// The directory the log files are written in.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// The most verbose level that reaches the file.
    pub fn level(&self) -> Level {
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asking_for_everything_still_stops_at_what_coffret_itself_emits() {
        let settings = LogSettings::new("/tmp/logs").with_level(Level::TRACE);

        // Nothing of coffret's is below DEBUG, so TRACE would only ever turn on
        // an HTTP stack's and a cloud SDK's own instrumentation.
        assert_eq!(settings.level(), Level::DEBUG);
        assert_eq!(
            LogSettings::new("/tmp/logs")
                .with_level(Level::WARN)
                .level(),
            Level::WARN,
        );
    }
}
