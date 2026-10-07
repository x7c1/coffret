use tauri::{AppHandle, Manager};

use super::WINDOW_LABEL;

/// Hides the Passphrase window, once what it asked for has been given.
///
/// Hidden rather than closed: it is the shell's one window, and it is what an
/// unlock brings forward again after the Library has locked.
pub(super) fn hide_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        if let Err(error) = window.hide() {
            tracing::warn!(%error, "the Passphrase window could not be hidden");
        }
    }
}
