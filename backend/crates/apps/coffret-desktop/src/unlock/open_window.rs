use anyhow::Context;
use tauri::{App, WebviewUrl, WebviewWindowBuilder};

use super::WINDOW_LABEL;

/// Opens the Passphrase window on `libraries`.
///
/// The names are handed to the page before its own script runs, as a frozen
/// list, so the page asks the shell for nothing but the one thing it is for.
pub fn open_window(app: &App, libraries: &[String]) -> anyhow::Result<()> {
    let names =
        serde_json::to_string(libraries).context("the Library names could not be encoded")?;
    WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::App("index.html".into()))
        .title("Coffret")
        .inner_size(380.0, 260.0)
        .resizable(false)
        .center()
        .initialization_script(format!(
            "window.COFFRET_LIBRARIES = Object.freeze({names});"
        ))
        .build()
        .context("the Passphrase window could not be opened")?;
    Ok(())
}
