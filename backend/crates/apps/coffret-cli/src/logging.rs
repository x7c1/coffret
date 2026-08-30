use anyhow::Context;
use coffret_logging::{install, LogSettings};

/// Points this run's events at the log file, and says which file that is.
///
/// A binary is what installs a subscriber; the library crates it drives only
/// emit. Where the file is is printed to standard error rather than logged: it
/// is a local path, and a local path is one of the things an event may not
/// carry.
pub fn start() -> anyhow::Result<()> {
    let settings = LogSettings::from_env().context("the log settings could not be read")?;
    let path = install(&settings).context("logging could not be started")?;
    eprintln!("Logging this run to {}.", path.display());
    Ok(())
}
