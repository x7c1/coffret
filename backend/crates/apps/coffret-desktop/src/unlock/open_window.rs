use anyhow::Context;
use tauri::{App, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use super::{Shell, WINDOW_LABEL};

/// Opens the Passphrase window on `libraries`.
///
/// The names are handed to the page before its own script runs, as a frozen
/// list, so the page asks the shell for nothing but the one thing it is for.
pub fn open_window(app: &App, libraries: &[String]) -> anyhow::Result<()> {
    let names =
        serde_json::to_string(libraries).context("the Library names could not be encoded")?;
    let window = WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::App("index.html".into()))
        .title("Coffret")
        .inner_size(380.0, 260.0)
        .resizable(false)
        .center()
        .initialization_script(format!(
            "window.COFFRET_LIBRARIES = Object.freeze({names});"
        ))
        .build()
        .context("the Passphrase window could not be opened")?;

    // Closing it once a Library is served hides it instead. It is the shell's
    // one window, and closing the last window ends the process — which would
    // be the server and its explorer gone because somebody dismissed the
    // Passphrase window shown for an unlock. Before then, closing it is how
    // somebody declines to open anything, and it ends the shell as it always
    // has.
    let hidden = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            if hidden.app_handle().state::<Shell>().explorer().is_some() {
                api.prevent_close();
                if let Err(error) = hidden.hide() {
                    tracing::warn!(%error, "the Passphrase window could not be hidden");
                }
            }
        }
    });
    Ok(())
}
