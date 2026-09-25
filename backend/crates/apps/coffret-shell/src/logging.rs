//! Where a run's events go.

use coffret_logging::{install, LogSettings};

use crate::error::{Error, Result};

/// Points this run's events at the log file, and says which file that is.
///
/// A binary is what installs a subscriber; the library crates it drives only
/// emit. Where the file is is printed to standard error rather than logged: it
/// is a local path, and a local path is one of the things an event may not
/// carry.
pub fn start() -> Result<()> {
    let settings = LogSettings::from_env().map_err(|cause| Error::LogSettingsUnread { cause })?;
    let path = install(&settings).map_err(|cause| Error::LogNotStarted { cause })?;
    eprintln!("Logging this run to {}.", path.display());
    Ok(())
}
