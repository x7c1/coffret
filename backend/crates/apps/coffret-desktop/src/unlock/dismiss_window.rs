use tauri::{AppHandle, Manager};

use super::WINDOW_LABEL;

/// Hides the Passphrase window, once the page has heard that what it asked
/// for was given.
///
/// A step the page asks for rather than one the open or the unlock takes,
/// because the page's call has to be answered while the page is still there to
/// hear it. A window hidden and reloaded with the call still waiting tears the
/// call down, and WebKitGTK sends it again once the page is back, with the same
/// Passphrase: the Library was opened twice over, and the explorer with it.
///
/// Only hidden here: the page loads itself afresh once this has answered, for
/// the same reason — a reload while this call waits would be this call sent
/// again. The fresh page is what empties the field, as every other hiding's
/// does ([`super::hide`]).
#[tauri::command]
pub fn dismiss_window(app: AppHandle) {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        tracing::warn!("the Passphrase window is not there to hide");
        return;
    };
    if let Err(error) = window.hide() {
        tracing::warn!(%error, "the Passphrase window could not be hidden");
    }
}
