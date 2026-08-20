use std::env;
use std::path::PathBuf;

use crate::error::{Error, Result};

use super::default_directory::default_directory;
use super::LogSettings;

/// Names the directory to log into, in place of the default one.
pub const LOG_DIRECTORY: &str = "COFFRET_LOG_DIR";
/// The level to log at, and the crates to keep beyond coffret's own.
///
/// A level by itself — `debug` — or a level and the extra targets to let
/// through after it: `debug,aws_smithy_runtime,hyper`.
pub const LOG_LEVEL: &str = "COFFRET_LOG";
/// Names the ceiling on the total bytes of log files kept, in place of the
/// default one.
pub const LOG_MAX_BYTES: &str = "COFFRET_LOG_MAX_BYTES";

impl LogSettings {
    /// Reads the settings an operator may override from the environment.
    ///
    /// [`LOG_DIRECTORY`] moves the logs, [`LOG_MAX_BYTES`] is the ceiling in
    /// bytes, and [`LOG_LEVEL`] is a level — one of `trace`, `debug`, `info`,
    /// `warn`, `error` — optionally followed by the crates to keep beyond
    /// coffret's own: `debug,aws_smithy_runtime`. A value that cannot be read
    /// is reported rather than ignored: silently logging somewhere else, or at
    /// a level nobody asked for, is worse than refusing to start.
    pub fn from_env() -> Result<Self> {
        // An empty value is a variable that was exported from something that
        // held nothing, not a request to log into the root of the filesystem —
        // the same reading `default_directory` gives `XDG_STATE_HOME`.
        let named = env::var_os(LOG_DIRECTORY).filter(|value| !value.is_empty());
        let directory = match named {
            Some(directory) => PathBuf::from(directory),
            None => default_directory()?,
        };
        let mut settings = Self::new(directory);

        if let Ok(setting) = env::var(LOG_LEVEL) {
            settings = read_level_setting(settings, &setting)?;
        }
        if let Ok(ceiling) = env::var(LOG_MAX_BYTES) {
            let bytes = match ceiling.parse() {
                Ok(bytes) => bytes,
                Err(cause) => {
                    return Err(Error::UnreadableCeiling {
                        value: ceiling,
                        cause,
                    })
                }
            };
            settings = settings.with_ceiling(bytes);
        }
        Ok(settings)
    }
}

/// Reads [`LOG_LEVEL`], which names a level and may name targets after it.
///
/// The level comes first because that is what the variable has always meant.
/// Anything after it is a crate to let through alongside coffret's own, for
/// somebody who has decided they want a dependency's account of itself and is
/// spending the ceiling on it deliberately.
fn read_level_setting(mut settings: LogSettings, setting: &str) -> Result<LogSettings> {
    let mut parts = setting.split(',').map(str::trim);
    let level = parts.next().unwrap_or_default();
    let parsed = level.parse().map_err(|cause| Error::UnreadableLevel {
        value: setting.to_owned(),
        cause,
    })?;
    settings = settings.with_level(parsed);

    for target in parts {
        if target.is_empty() {
            return Err(Error::EmptyTarget {
                value: setting.to_owned(),
            });
        }
        settings = settings.with_target(target);
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use tracing::Level;

    use super::*;

    #[test]
    fn widening_the_targets_is_not_a_way_past_the_level_cap() {
        let settings = read_level_setting(
            LogSettings::new("/tmp/logs"),
            "trace,aws_smithy_runtime,hyper_util",
        )
        .expect("the setting must be readable");

        // The targets widened; the level did not. TRACE is where a signer
        // prints its signing material, and no setting reaches it.
        assert_eq!(settings.level(), Level::DEBUG);
        assert!(settings.keeps("aws_smithy_runtime::client::orchestrator"));
        assert!(settings.keeps("hyper_util::client::legacy::connect::http"));
    }

    #[test]
    fn a_level_on_its_own_still_means_what_it_always_did() {
        let settings = read_level_setting(LogSettings::new("/tmp/logs"), "warn")
            .expect("the setting must be readable");

        assert_eq!(settings.level(), Level::WARN);
        assert!(!settings.keeps("aws_smithy_runtime::client::orchestrator"));
    }

    #[test]
    fn a_setting_that_cannot_be_read_is_reported_rather_than_ignored() {
        // What is wrong differs between them, and which of the two it was is
        // what the person who set the variable has to be told.
        let unnamed = rejected("not-a-level");
        assert!(
            matches!(unnamed, Error::UnreadableLevel { .. }),
            "{unnamed:?}"
        );

        let empty = rejected("");
        assert!(matches!(empty, Error::UnreadableLevel { .. }), "{empty:?}");

        let comma = rejected("debug,");
        assert!(matches!(comma, Error::EmptyTarget { .. }), "{comma:?}");

        let blank = rejected("debug, ,hyper_util");
        assert!(matches!(blank, Error::EmptyTarget { .. }), "{blank:?}");
    }

    /// What a setting that cannot be read is reported with.
    fn rejected(setting: &str) -> Error {
        read_level_setting(LogSettings::new("/tmp/logs"), setting)
            .expect_err("an unreadable setting must be reported")
    }
}
