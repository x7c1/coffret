use tauri::{AppHandle, Manager};

use super::WINDOW_LABEL;

/// Brings the Passphrase window forward, where it is still up.
pub fn bring_forward(app: &AppHandle) {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        tracing::info!("coffret-desktop was launched again before its window was open");
        return;
    };
    for (step, result) in [
        ("unminimize", window.unminimize()),
        ("show", window.show()),
        ("focus", window.set_focus()),
    ] {
        if let Err(error) = result {
            tracing::warn!(%error, "could not {step} the Passphrase window on a second launch");
        }
    }
}
