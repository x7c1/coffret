//! What the shell does before anybody has typed anything.

use anyhow::Context;
use coffret_device::LibraryDir;
use tauri::{App, Manager};

use crate::settings;
use crate::unlock::{self, Shell};

/// Starts the log, reads the setting the server will need, and opens the
/// Passphrase window on the Libraries this device has.
///
/// The setting is read here rather than when a Library is opened, so that one
/// a person has to fix is said before they have typed a Passphrase for nothing.
pub fn start(app: &App) -> anyhow::Result<()> {
    coffret_shell::logging::start().context("the log could not be started")?;

    let idle_minutes = settings::idle_minutes()?;
    app.state::<Shell>().set_idle_minutes(idle_minutes);

    let libraries: Vec<String> = LibraryDir::on_this_device()
        .context("the Libraries on this device could not be listed")?
        .iter()
        .map(|dir| dir.name().to_owned())
        .collect();
    unlock::open_window(app, &libraries)
}
