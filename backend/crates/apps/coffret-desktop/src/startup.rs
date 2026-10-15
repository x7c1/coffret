//! What the app does before anybody has typed anything.

use anyhow::Context;
use coffret_device::LibraryDir;
use tauri::{App, Manager};

use crate::launch::{launch_dir_path, prepare_launch_dir};
use crate::settings;
use crate::unlock::{self, Desktop};

/// Starts the log, reads the setting the server will need, prepares the
/// directory launch files go in, and opens the Passphrase window on the
/// Libraries this device has.
///
/// The setting and the directory are taken here rather than when a Library is
/// opened, so that either one a person has to fix is said before they have
/// typed a Passphrase for nothing. Preparing the directory also sweeps away
/// what the app's last run left in it (spec: LA-21).
pub fn start(app: &App) -> anyhow::Result<()> {
    coffret_shell::logging::start().context("the log could not be started")?;

    let idle_minutes = settings::idle_minutes()?;
    app.state::<Desktop>().set_idle_minutes(idle_minutes);

    let launch_dir = launch_dir_path(&app.config().identifier)?;
    app.state::<Desktop>()
        .set_launch_dir(prepare_launch_dir(&launch_dir)?);

    let libraries: Vec<String> = LibraryDir::on_this_device()
        .context("the Libraries on this device could not be listed")?
        .iter()
        .map(|dir| dir.name().to_owned())
        .collect();
    unlock::open_window(app, &libraries)
}
