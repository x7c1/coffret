use tauri::{AppHandle, Manager, WebviewWindow};

use super::WINDOW_LABEL;

/// Hides the Passphrase window, once what it asked for has been given.
///
/// Hidden rather than closed: it is the shell's one window, and it is what an
/// unlock brings forward again after the Library has locked.
pub(super) fn hide_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        hide(&window);
    }
}

/// Hides `window` and loads its page afresh, which empties the Passphrase field.
///
/// Every way the window goes out of sight comes through here — an open or an
/// unlock that landed, and a dismissal once a Library is served — so that
/// nothing typed stays in a page nobody is looking at. The page empties the
/// field itself after every submission, but a dismissal is not a submission:
/// without this, what somebody typed and then thought better of would sit in
/// the hidden page until the next asking loaded it again.
///
/// Reloading drops the page's script state along with the field, the string the
/// last call was made with included. That shortens how long the webview's copies
/// live; it does not overwrite them, and nothing coffret runs can — they are
/// outside what DK-7 claims.
pub(super) fn hide(window: &WebviewWindow) {
    if let Err(error) = window.hide() {
        tracing::warn!(%error, "the Passphrase window could not be hidden");
    }
    // After hiding, so the page reloading is not seen; the same page, so the
    // window comes back in the mode it left in.
    if let Err(error) = window.reload() {
        tracing::warn!(%error, "the Passphrase window's page could not be emptied");
    }
}
